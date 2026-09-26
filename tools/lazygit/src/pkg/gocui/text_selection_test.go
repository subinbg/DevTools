package gocui

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func newTestViewWithContent(t *testing.T, width int, wrap bool, content string) *View {
	t.Helper()
	v := NewView("test", 0, 0, width+1, 10, OutputNormal)
	v.Wrap = wrap
	v.SetContent(content)
	return v
}

func TestTextSelection(t *testing.T) {
	v := newTestViewWithContent(t, 20, false, "first line\nsecond line\nthird line")

	assert.False(t, v.HasTextSelection())
	assert.Equal(t, "", v.SelectedText())

	v.StartTextSelection(6, 0)
	assert.False(t, v.HasTextSelection(), "a press alone selects nothing")

	v.ExtendTextSelection(5, 1)
	assert.True(t, v.HasTextSelection())
	assert.Equal(t, "line\nsecond", v.SelectedText())

	// dragging upwards selects the same text
	v.StartTextSelection(5, 1)
	v.ExtendTextSelection(6, 0)
	assert.Equal(t, "line\nsecond", v.SelectedText())

	// beyond the end of the content
	v.StartTextSelection(0, 2)
	v.ExtendTextSelection(100, 7)
	assert.Equal(t, "third line", v.SelectedText())

	assert.True(t, v.inTextSelection(0, 2))
	assert.True(t, v.inTextSelection(9, 2))
	assert.False(t, v.inTextSelection(9, 1))

	v.ClearTextSelection()
	assert.False(t, v.HasTextSelection())
	assert.False(t, v.inTextSelection(0, 2))
}

func TestTextSelectionJoinsWrappedLines(t *testing.T) {
	v := newTestViewWithContent(t, 10, true, "0123456789abcdef\nnext")

	assert.Equal(t, 3, v.ViewLinesHeight())

	v.StartTextSelection(5, 0)
	v.ExtendTextSelection(1, 2)
	assert.Equal(t, "56789abcdef\nne", v.SelectedText())
}

func TestTextSelectionClearedOnReset(t *testing.T) {
	v := newTestViewWithContent(t, 20, false, "some text")
	v.StartTextSelection(0, 0)
	v.ExtendTextSelection(3, 0)
	assert.True(t, v.HasTextSelection())

	v.Reset()
	assert.False(t, v.HasTextSelection())
}
