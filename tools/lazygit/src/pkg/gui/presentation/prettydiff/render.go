package prettydiff

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/jesseduffield/lazygit/pkg/gui/style"
)

// The gutter of a diff line: the old and new line numbers, each right-aligned
// in numWidth columns, then the separator; the marker (' ', '+' or '-') and
// the text follow.
const (
	gutterSeparator = "│"
	minNumWidth     = 3
	// shown in the gutter of a hunk header
	hunkGutterMark = "⋯"
	noNewlineText  = "↵ No newline at end of file"
)

// Renderer renders the lines of one file's diff.
type Renderer struct {
	theme    Theme
	numWidth int
	// false renders the same layout without colors, e.g. to measure how the
	// lines wrap
	colors bool
}

// NewRenderer returns a renderer whose gutter fits line numbers up to
// maxLineNumber.
func NewRenderer(theme Theme, maxLineNumber int, colors bool) *Renderer {
	return &Renderer{
		theme:    theme,
		numWidth: NumWidth(maxLineNumber),
		colors:   colors,
	}
}

// NumWidth is the width of a line-number column fitting the given number.
func NumWidth(maxLineNumber int) int {
	return max(minNumWidth, len(strconv.Itoa(max(maxLineNumber, 0))))
}

// GutterWidth is the number of cells before the marker of a diff line, for
// the given line-number column width.
func GutterWidth(numWidth int) int {
	return 2*numWidth + 2 + len([]rune(gutterSeparator))
}

func (r *Renderer) sprint(s style.TextStyle, text string) string {
	if !r.colors {
		return text
	}
	return s.Sprint(text)
}

// fill extends the styling of the last styled run to the end of the line, so
// that a tinted line is tinted all the way across the panel.
func (r *Renderer) fill(styled string) string {
	if !r.colors {
		return styled
	}
	const reset = "\x1b[0m"
	if strings.HasSuffix(styled, reset) {
		return styled[:len(styled)-len(reset)] + "\x1b[K" + reset
	}
	return styled
}

func (r *Renderer) number(n int) string {
	if n <= 0 {
		return strings.Repeat(" ", r.numWidth)
	}
	return fmt.Sprintf("%*d", r.numWidth, n)
}

func (r *Renderer) gutter(s style.TextStyle, oldNo, newNo int) string {
	return r.sprint(s, r.number(oldNo)+" "+r.number(newNo)+" "+gutterSeparator)
}

// Context renders an unchanged line.
func (r *Renderer) Context(oldNo, newNo int, text string) string {
	return r.gutter(r.theme.styles.gutter, oldNo, newNo) + " " + text
}

// Add renders an added line. emph marks the changed words; included marks
// the line as part of a custom patch being built.
func (r *Renderer) Add(newNo int, text string, emph []Span, included bool) string {
	s := r.theme.styles
	marker := s.addMarker
	if included {
		marker = s.addIncluded
	}
	return r.fill(r.gutter(s.addGutter, 0, newNo) + r.sprint(marker, "+") + r.text(text, emph, s.addText, s.addEmph))
}

// Del renders a removed line.
func (r *Renderer) Del(oldNo int, text string, emph []Span, included bool) string {
	s := r.theme.styles
	marker := s.delMarker
	if included {
		marker = s.delIncluded
	}
	return r.fill(r.gutter(s.delGutter, oldNo, 0) + r.sprint(marker, "-") + r.text(text, emph, s.delText, s.delEmph))
}

func (r *Renderer) text(text string, emph []Span, base style.TextStyle, emphStyle style.TextStyle) string {
	if len(emph) == 0 {
		return r.sprint(base, text)
	}
	builder := strings.Builder{}
	pos := 0
	for _, span := range emph {
		if span.Start < pos || span.End > len(text) || span.Start >= span.End {
			continue
		}
		if span.Start > pos {
			builder.WriteString(r.sprint(base, text[pos:span.Start]))
		}
		builder.WriteString(r.sprint(emphStyle, text[span.Start:span.End]))
		pos = span.End
	}
	// keep the base style last so that the fill extends it
	builder.WriteString(r.sprint(base, text[pos:]))
	return builder.String()
}

// HunkHeader renders a hunk header line, e.g. "@@ -1,3 +1,4 @@ func main()".
func (r *Renderer) HunkHeader(header string) string {
	width := GutterWidth(r.numWidth)
	left := (width - 1) / 2
	gutter := strings.Repeat(" ", left) + hunkGutterMark + strings.Repeat(" ", width-left-1)
	return r.fill(r.sprint(r.theme.styles.hunk, gutter+" "+header))
}

// FileHeader renders the line introducing a file's diff.
func (r *Renderer) FileHeader(path string, added int, removed int) string {
	s := r.theme.styles
	line := r.sprint(s.file, " "+path+"  ")
	if added > 0 || removed > 0 {
		line += r.sprint(s.fileAdded, fmt.Sprintf("+%d", added)) + r.sprint(s.file, " ") +
			r.sprint(s.fileRemoved, fmt.Sprintf("−%d", removed))
	}
	return r.fill(line)
}

// Meta renders a line of information about the file, shown under its header
// ("new file", "renamed from ...").
func (r *Renderer) Meta(text string) string {
	return r.fill(r.sprint(r.theme.styles.meta, " "+text))
}

// NoNewline renders git's "\ No newline at end of file" marker.
func (r *Renderer) NoNewline() string {
	return r.gutter(r.theme.styles.gutter, 0, 0) + " " + r.sprint(r.theme.styles.noNL, noNewlineText)
}

// Blank renders an empty separator line.
func (r *Renderer) Blank() string {
	return ""
}
