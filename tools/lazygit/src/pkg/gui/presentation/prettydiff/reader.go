package prettydiff

import (
	"bufio"
	"io"
	"regexp"
	"strconv"
	"strings"
)

var (
	ansiRegexp       = regexp.MustCompile(`\x1b\[[0-9;?]*[A-Za-z]`)
	hunkHeaderRegexp = regexp.MustCompile(`^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@(.*)$`)
)

// StripANSI removes terminal escape sequences from a line.
func StripANSI(s string) string {
	if !strings.Contains(s, "\x1b") {
		return s
	}
	return ansiRegexp.ReplaceAllString(s, "")
}

// Reader formats the unified diffs found in the output of a git command, line
// by line, and passes everything else (commit headers, stat summaries, log
// output) through unchanged. Each file's diff is formatted once it has been
// read completely, so that its header can show the counts of added and
// removed lines.
type Reader struct {
	src   *bufio.Reader
	theme Theme

	out    []byte
	outPos int
	done   bool

	inSection bool
	section   []string
	fileCount int
}

// NewReader wraps the output of a git command.
func NewReader(r io.Reader, theme Theme) *Reader {
	return &Reader{src: bufio.NewReaderSize(r, 64*1024), theme: theme}
}

func (r *Reader) Read(p []byte) (int, error) {
	for r.outPos >= len(r.out) {
		if r.done {
			return 0, io.EOF
		}
		r.out = r.out[:0]
		r.outPos = 0
		r.fill()
	}
	n := copy(p, r.out[r.outPos:])
	r.outPos += n
	return n, nil
}

func (r *Reader) fill() {
	line, err := r.src.ReadString('\n')
	if len(line) > 0 {
		line = strings.TrimSuffix(line, "\n")
		line = strings.TrimSuffix(line, "\r")
		r.processLine(line)
	}
	if err != nil {
		r.flushSection()
		r.done = true
	}
}

func (r *Reader) emit(line string) {
	r.out = append(r.out, line...)
	r.out = append(r.out, '\n')
}

func isSectionStart(plain string) bool {
	return strings.HasPrefix(plain, "diff --git ") ||
		strings.HasPrefix(plain, "diff --cc ") ||
		strings.HasPrefix(plain, "diff --combined ")
}

func (r *Reader) processLine(line string) {
	plain := StripANSI(line)
	if isSectionStart(plain) {
		r.flushSection()
		r.inSection = true
		r.section = []string{line}
		return
	}
	if r.inSection {
		r.section = append(r.section, line)
		return
	}
	r.emit(line)
}

func (r *Reader) flushSection() {
	if !r.inSection {
		return
	}
	for _, line := range FormatSection(r.section, r.theme, r.fileCount > 0) {
		r.emit(line)
	}
	r.fileCount++
	r.inSection = false
	r.section = nil
}

type hunk struct {
	header                 string
	oldStart, oldCount     int
	newStart, newCount     int
	lines                  []string // plain body lines, including the marker
	numAdded, numRemoved   int
	oldLineMax, newLineMax int
}

// FormatSection formats one file's diff: the lines from its "diff --git"
// line up to the next file. Diffs that are not plain unified diffs of one
// file (combined diffs of merges, binary patches) are returned as they are.
// separate adds an empty line before the header, to separate the file from
// the previous one.
func FormatSection(rawLines []string, theme Theme, separate bool) []string {
	lines := make([]string, len(rawLines))
	for i, line := range rawLines {
		lines[i] = StripANSI(line)
	}
	if len(lines) == 0 {
		return nil
	}

	first := lines[0]
	if !strings.HasPrefix(first, "diff --git ") {
		return rawLines
	}

	hunkStart := len(lines)
	for i, line := range lines {
		if strings.HasPrefix(line, "@@") {
			hunkStart = i
			break
		}
	}
	for _, line := range lines[1:hunkStart] {
		if strings.HasPrefix(line, "GIT binary patch") {
			return rawLines
		}
	}

	path, metas := ParseExtendedHeader(first, lines[1:hunkStart])
	hunks := parseHunks(lines[hunkStart:])

	added, removed, maxLine := 0, 0, 0
	for _, h := range hunks {
		added += h.numAdded
		removed += h.numRemoved
		maxLine = max(maxLine, h.oldLineMax, h.newLineMax)
	}

	renderer := NewRenderer(theme, maxLine, true)

	result := make([]string, 0, len(lines)+4)
	if separate {
		result = append(result, renderer.Blank())
	}
	result = append(result, renderer.FileHeader(path, added, removed))
	for _, meta := range metas {
		result = append(result, renderer.Meta(meta))
	}
	for _, h := range hunks {
		result = append(result, renderer.HunkHeader(h.header))
		result = append(result, renderHunkBody(renderer, h.lines, h.oldStart, h.newStart)...)
	}
	return result
}

// ParseExtendedHeader reads the lines between "diff --git" and the first hunk
// and returns the file's path and the facts worth showing about it ("new
// file", "renamed from ...").
func ParseExtendedHeader(diffLine string, header []string) (string, []string) {
	usesPrefixes := strings.HasPrefix(strings.TrimPrefix(diffLine, "diff --git "), "a/")
	stripPrefix := func(path string, prefix string) string {
		if usesPrefixes {
			return strings.TrimPrefix(path, prefix)
		}
		return path
	}

	var oldPath, newPath, renameFrom, copyFrom, oldMode string
	var metas []string
	for _, line := range header {
		switch {
		case strings.HasPrefix(line, "--- "):
			oldPath = strings.TrimPrefix(line, "--- ")
		case strings.HasPrefix(line, "+++ "):
			newPath = strings.TrimPrefix(line, "+++ ")
		case strings.HasPrefix(line, "new file mode "):
			metas = append(metas, "new file")
		case strings.HasPrefix(line, "deleted file mode "):
			metas = append(metas, "deleted file")
		case strings.HasPrefix(line, "rename from "):
			renameFrom = strings.TrimPrefix(line, "rename from ")
		case strings.HasPrefix(line, "rename to "):
			metas = append(metas, "renamed from "+renameFrom)
		case strings.HasPrefix(line, "copy from "):
			copyFrom = strings.TrimPrefix(line, "copy from ")
		case strings.HasPrefix(line, "copy to "):
			metas = append(metas, "copied from "+copyFrom)
		case strings.HasPrefix(line, "old mode "):
			oldMode = strings.TrimPrefix(line, "old mode ")
		case strings.HasPrefix(line, "new mode "):
			metas = append(metas, "mode changed "+oldMode+" → "+strings.TrimPrefix(line, "new mode "))
		case strings.HasPrefix(line, "Binary files "):
			metas = append(metas, "binary file changed")
		case strings.HasPrefix(line, "similarity index "),
			strings.HasPrefix(line, "dissimilarity index "),
			strings.HasPrefix(line, "index "),
			line == "":
			// nothing worth showing
		default:
			metas = append(metas, line)
		}
	}

	path := ""
	switch {
	case newPath != "" && newPath != "/dev/null":
		path = stripPrefix(newPath, "b/")
	case oldPath != "" && oldPath != "/dev/null":
		path = stripPrefix(oldPath, "a/")
	default:
		rest := strings.TrimPrefix(diffLine, "diff --git ")
		if i := strings.Index(rest, " b/"); usesPrefixes && i >= 0 {
			path = rest[i+len(" b/"):]
		} else if i := strings.Index(rest, " "); i >= 0 {
			path = rest[i+1:]
		} else {
			path = rest
		}
	}
	return path, metas
}

func parseHunks(lines []string) []hunk {
	var hunks []hunk
	for _, line := range lines {
		if match := hunkHeaderRegexp.FindStringSubmatch(line); match != nil {
			h := hunk{
				header:   line,
				oldStart: atoi(match[1]),
				oldCount: atoiDefault(match[2], 1),
				newStart: atoi(match[3]),
				newCount: atoiDefault(match[4], 1),
			}
			h.oldLineMax = h.oldStart + h.oldCount
			h.newLineMax = h.newStart + h.newCount
			hunks = append(hunks, h)
			continue
		}
		if len(hunks) == 0 {
			continue
		}
		h := &hunks[len(hunks)-1]
		h.lines = append(h.lines, line)
		switch {
		case strings.HasPrefix(line, "+"):
			h.numAdded++
		case strings.HasPrefix(line, "-"):
			h.numRemoved++
		}
	}
	return hunks
}

func atoi(s string) int {
	n, _ := strconv.Atoi(s)
	return n
}

func atoiDefault(s string, def int) int {
	if s == "" {
		return def
	}
	return atoi(s)
}

// A block is a run of removed lines followed by a run of added lines (with
// possibly a "no newline" marker in between), whose lines are compared word
// by word.
type blockLine struct {
	marker byte
	text   string
}

func renderHunkBody(renderer *Renderer, lines []string, oldStart int, newStart int) []string {
	result := make([]string, 0, len(lines))
	oldNo, newNo := oldStart, newStart

	var block []blockLine
	flushBlock := func() {
		if len(block) == 0 {
			return
		}
		var dels, adds []string
		for _, l := range block {
			switch l.marker {
			case '-':
				dels = append(dels, l.text)
			case '+':
				adds = append(adds, l.text)
			}
		}
		delSpans, addSpans := BlockSpans(dels, adds)
		di, ai := 0, 0
		for _, l := range block {
			switch l.marker {
			case '-':
				result = append(result, renderer.Del(oldNo, l.text, delSpans[di], false))
				di++
				oldNo++
			case '+':
				result = append(result, renderer.Add(newNo, l.text, addSpans[ai], false))
				ai++
				newNo++
			default:
				result = append(result, renderer.NoNewline())
			}
		}
		block = nil
	}

	for _, line := range lines {
		marker := byte(' ')
		text := ""
		if line != "" {
			marker = line[0]
			text = line[1:]
		}
		switch marker {
		case '-', '+':
			block = append(block, blockLine{marker: marker, text: text})
		case '\\':
			if len(block) > 0 {
				block = append(block, blockLine{marker: marker})
			} else {
				result = append(result, renderer.NoNewline())
			}
		case ' ':
			flushBlock()
			result = append(result, renderer.Context(oldNo, newNo, text))
			oldNo++
			newNo++
		default:
			flushBlock()
			result = append(result, line)
		}
	}
	flushBlock()
	return result
}
