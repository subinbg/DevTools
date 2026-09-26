package prettydiff

import (
	"io"
	"strings"
	"testing"

	"github.com/gookit/color"
	"github.com/stretchr/testify/assert"
	"github.com/xo/terminfo"
)

func TestWordSpans(t *testing.T) {
	scenarios := []struct {
		name     string
		oldLine  string
		newLine  string
		oldSpans []Span
		newSpans []Span
		ok       bool
	}{
		{
			name:     "one word changed",
			oldLine:  "fmt.Println(\"old\")",
			newLine:  "fmt.Println(\"new\")",
			oldSpans: []Span{{13, 16}},
			newSpans: []Span{{13, 16}},
			ok:       true,
		},
		{
			name:     "a word inserted",
			oldLine:  "return err",
			newLine:  "return nil, err",
			oldSpans: nil,
			newSpans: []Span{{7, 12}},
			ok:       true,
		},
		{
			name:    "completely different lines get no highlight",
			oldLine: "alpha beta gamma",
			newLine: "one two three four",
			ok:      false,
		},
		{
			name:    "identical lines",
			oldLine: "same",
			newLine: "same",
			ok:      true,
		},
		{
			name:     "whitespace only change",
			oldLine:  "a  b",
			newLine:  "a b",
			oldSpans: []Span{{1, 3}},
			newSpans: []Span{{1, 2}},
			ok:       true,
		},
	}

	for _, s := range scenarios {
		t.Run(s.name, func(t *testing.T) {
			oldSpans, newSpans, ok := WordSpans(s.oldLine, s.newLine)
			assert.Equal(t, s.ok, ok)
			assert.Equal(t, s.oldSpans, oldSpans)
			assert.Equal(t, s.newSpans, newSpans)
		})
	}
}

func TestBlockSpansPairsBySimilarity(t *testing.T) {
	dels := []string{"x := compute(a)"}
	adds := []string{"// compute the thing", "x := compute(a, b)", "log(x)"}
	delSpans, addSpans := BlockSpans(dels, adds)
	assert.Nil(t, delSpans[0], "everything in the removed line is still there")
	assert.Nil(t, addSpans[0])
	assert.Equal(t, []Span{{14, 17}}, addSpans[1], "', b' was inserted")
	assert.Nil(t, addSpans[2])
}

func TestRenderer(t *testing.T) {
	oldColorLevel := color.ForceSetColorLevel(terminfo.ColorLevelNone)
	defer color.ForceSetColorLevel(oldColorLevel)

	r := NewRenderer(ThemeByName("dark"), 12, true)
	assert.Equal(t, "  1   1 │ context", r.Context(1, 1, "context"))
	assert.Equal(t, "      2 │+added", r.Add(2, "added", nil, false))
	assert.Equal(t, "  3     │-removed", r.Del(3, "removed", nil, false))
	assert.Equal(t, "    ⋯     @@ -1,2 +1,3 @@ func f()", r.HunkHeader("@@ -1,2 +1,3 @@ func f()"))
	assert.Equal(t, " path/to/file.go  +2 −1", r.FileHeader("path/to/file.go", 2, 1))
	assert.Equal(t, " path/to/file.go  ", r.FileHeader("path/to/file.go", 0, 0))
	assert.Equal(t, " renamed from old.go", r.Meta("renamed from old.go"))
	assert.Equal(t, "        │ "+noNewlineText, r.NoNewline())

	wide := NewRenderer(ThemeByName("light"), 12345, true)
	assert.Equal(t, "12345       │-x", wide.Del(12345, "x", nil, false))
}

func TestRendererStylesEmphasisAndFill(t *testing.T) {
	oldColorLevel := color.ForceSetColorLevel(terminfo.ColorLevelMillions)
	defer color.ForceSetColorLevel(oldColorLevel)

	r := NewRenderer(ThemeByName("dark"), 1, true)
	line := r.Add(1, "abc", []Span{{1, 2}}, false)
	assert.True(t, strings.HasSuffix(line, "\x1b[K\x1b[0m"), "tinted lines fill to the end: %q", line)
	assert.Contains(t, line, "a\x1b[0m")
	assert.Contains(t, line, "b\x1b[0m")
	assert.Equal(t, "abc", StripANSI(strings.TrimPrefix(StripANSI(line), "      1 │+")))

	plain := NewRenderer(ThemeByName("dark"), 1, false)
	assert.Equal(t, "      1 │+abc", plain.Add(1, "abc", []Span{{1, 2}}, true))
}

func format(t *testing.T, input string) string {
	t.Helper()
	out, err := io.ReadAll(NewReader(strings.NewReader(input), ThemeByName("dark")))
	assert.NoError(t, err)
	return string(out)
}

func TestReaderPassesNonDiffOutputThrough(t *testing.T) {
	input := "\x1b[33mcommit abc\x1b[m\nAuthor: me\n\n    subject\n\n file.go | 2 +-\n"
	assert.Equal(t, input, format(t, input))
	assert.Equal(t, "", format(t, ""))
	assert.Equal(t, "no trailing newline\n", format(t, "no trailing newline"))
}

func TestReaderFormatsADiff(t *testing.T) {
	oldColorLevel := color.ForceSetColorLevel(terminfo.ColorLevelNone)
	defer color.ForceSetColorLevel(oldColorLevel)

	input := strings.Join([]string{
		"commit abc",
		"",
		"\x1b[1mdiff --git a/main.go b/main.go\x1b[m",
		"index 1234567..89abcde 100644",
		"--- a/main.go",
		"+++ b/main.go",
		"\x1b[36m@@ -1,4 +1,5 @@\x1b[m func main() {",
		" import \"fmt\"",
		"\x1b[31m-\tfmt.Println(\"old\")\x1b[m",
		"\x1b[32m+\tfmt.Println(\"new\")\x1b[m",
		"\x1b[32m+\tfmt.Println(\"more\")\x1b[m",
		" }",
		"\\ No newline at end of file",
		"diff --git a/new.txt b/new.txt",
		"new file mode 100644",
		"index 0000000..e69de29",
		"--- /dev/null",
		"+++ b/new.txt",
		"@@ -0,0 +1 @@",
		"+hello",
		"",
	}, "\n")

	expected := strings.Join([]string{
		"commit abc",
		"",
		" main.go  +2 −1",
		"    ⋯     @@ -1,4 +1,5 @@ func main() {",
		"  1   1 │ import \"fmt\"",
		"  2     │-\tfmt.Println(\"old\")",
		"      2 │+\tfmt.Println(\"new\")",
		"      3 │+\tfmt.Println(\"more\")",
		"  3   4 │ }",
		"        │ " + noNewlineText,
		"",
		" new.txt  +1 −0",
		" new file",
		"    ⋯     @@ -0,0 +1 @@",
		"      1 │+hello",
		"",
	}, "\n")
	assert.Equal(t, expected, format(t, input))
}

func TestReaderLeavesCombinedAndBinaryDiffsAlone(t *testing.T) {
	combined := "diff --cc a.txt\nindex 1,2..3\n--- a/a.txt\n+++ b/a.txt\n@@@ -1,2 -1,2 +1,3 @@@\n  x\n++y\n"
	assert.Equal(t, combined, format(t, combined))

	binary := "diff --git a/b.png b/b.png\nGIT binary patch\nliteral 3\nKcmZQzWMT\n\nliteral 0\nHcmV?d00001\n\n"
	assert.Equal(t, binary, format(t, binary))
}

func TestReaderDescribesBinaryAndRenamedFiles(t *testing.T) {
	oldColorLevel := color.ForceSetColorLevel(terminfo.ColorLevelNone)
	defer color.ForceSetColorLevel(oldColorLevel)

	input := strings.Join([]string{
		"diff --git a/img.png b/img.png",
		"index 1234567..89abcde 100644",
		"Binary files a/img.png and b/img.png differ",
		"diff --git a/old.go b/new.go",
		"similarity index 90%",
		"rename from old.go",
		"rename to new.go",
		"old mode 100644",
		"new mode 100755",
		"",
	}, "\n")
	expected := strings.Join([]string{
		" img.png  ",
		" binary file changed",
		"",
		" new.go  ",
		" renamed from old.go",
		" mode changed 100644 → 100755",
		"",
	}, "\n")
	assert.Equal(t, expected, format(t, input))
}

func TestParseExtendedHeaderPaths(t *testing.T) {
	path, _ := ParseExtendedHeader("diff --git a/dir/x.go b/dir/x.go", []string{"--- a/dir/x.go", "+++ b/dir/x.go"})
	assert.Equal(t, "dir/x.go", path)

	path, _ = ParseExtendedHeader("diff --git dir/x.go dir/x.go", []string{"--- dir/x.go", "+++ dir/x.go"})
	assert.Equal(t, "dir/x.go", path, "no prefixes with diff.noprefix")

	path, _ = ParseExtendedHeader("diff --git a/gone.go b/gone.go", []string{"deleted file mode 100644", "--- a/gone.go", "+++ /dev/null"})
	assert.Equal(t, "gone.go", path)

	path, _ = ParseExtendedHeader("diff --git a/b.png b/b.png", []string{"Binary files a/b.png and b/b.png differ"})
	assert.Equal(t, "b.png", path)
}

func TestReaderReadsInSmallChunks(t *testing.T) {
	input := "x\ndiff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -1 +1 @@\n-1\n+2\n"
	r := NewReader(strings.NewReader(input), ThemeByName("dark"))
	buf := make([]byte, 3)
	var out []byte
	for {
		n, err := r.Read(buf)
		out = append(out, buf[:n]...)
		if err == io.EOF {
			break
		}
		assert.NoError(t, err)
	}
	assert.Equal(t, format(t, input), string(out))
}
