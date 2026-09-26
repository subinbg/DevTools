// Package palette provides a fixed set of distinct colors used to color
// commit-graph lanes (cyclically by lane position), the ref labels shown on
// commits, and branch names (stably by name hash, so a branch keeps its color
// across renders; with more branches than colors, some branches share one).
package palette

import (
	"hash/fnv"
	"strings"

	"github.com/gookit/color"
	"github.com/jesseduffield/lazygit/pkg/gui/style"
)

type rgb struct{ r, g, b uint8 }

// laneColors is the predefined palette (N = 10), ordered so that adjacent
// indices (= adjacent graph lanes) contrast strongly.
var laneColors = []rgb{
	{0x61, 0xaf, 0xef}, // blue
	{0x98, 0xc3, 0x79}, // green
	{0xe0, 0x6c, 0x75}, // red
	{0xe5, 0xc0, 0x7b}, // yellow
	{0xc6, 0x78, 0xdd}, // purple
	{0x56, 0xb6, 0xc2}, // cyan
	{0xff, 0x9e, 0x64}, // orange
	{0xf4, 0x7f, 0xd4}, // pink
	{0x1a, 0xbc, 0x9c}, // teal
	{0x9d, 0x7c, 0xd8}, // violet
}

var (
	labelTextDark  = rgb{0x1e, 0x22, 0x27}
	labelTextLight = rgb{0xec, 0xee, 0xf1}
	tagColor       = rgb{0xd9, 0xa5, 0x2b} // gold
	headColor      = rgb{0xe0, 0x6c, 0x75} // red, for a detached HEAD
)

func toColor(c rgb) style.Color {
	return style.NewRGBColor(color.RGB(c.r, c.g, c.b))
}

func fgStyle(c rgb) style.TextStyle {
	return style.New().SetFg(toColor(c))
}

// labelStyle is a filled "pill": text in fg on a bg-colored background.
func labelStyle(bg rgb, fg rgb) style.TextStyle {
	return style.New().SetBg(toColor(bg)).SetFg(toColor(fg))
}

func dimmed(c rgb) rgb {
	const factor = 0.55
	return rgb{uint8(float64(c.r) * factor), uint8(float64(c.g) * factor), uint8(float64(c.b) * factor)}
}

var (
	// one shared pointer per lane color, so that a style handed out by ByIndex
	// can be mapped back to its index with IndexOf
	laneStyles     []*style.TextStyle
	labelStyles    []style.TextStyle
	dimLabelStyles []style.TextStyle

	// TagLabel is the label style for tags.
	TagLabel = labelStyle(tagColor, labelTextDark)
	// HeadLabel is the label style for a detached HEAD.
	HeadLabel = labelStyle(headColor, labelTextDark)
)

func init() {
	for _, c := range laneColors {
		s := fgStyle(c)
		laneStyles = append(laneStyles, &s)
		labelStyles = append(labelStyles, labelStyle(c, labelTextDark))
		dimLabelStyles = append(dimLabelStyles, labelStyle(dimmed(c), labelTextLight))
	}
}

// Len returns the number of colors in the palette.
func Len() int {
	return len(laneColors)
}

func wrap(i int) int {
	n := len(laneColors)
	return ((i % n) + n) % n
}

// ByIndex returns the lane color for an index, wrapping cyclically. The same
// pointer is returned for the same index.
func ByIndex(i int) *style.TextStyle {
	return laneStyles[wrap(i)]
}

// IndexOf maps a style obtained from ByIndex back to its palette index.
func IndexOf(s *style.TextStyle) (int, bool) {
	for i, laneStyle := range laneStyles {
		if laneStyle == s {
			return i, true
		}
	}
	return 0, false
}

// IndexForName returns a stable palette index for a ref name. The remote
// prefix "origin/" is stripped so a branch and its origin counterpart match.
func IndexForName(name string) int {
	name = strings.TrimPrefix(name, "origin/")
	h := fnv.New32a()
	_, _ = h.Write([]byte(name))
	return int(h.Sum32() % uint32(len(laneColors)))
}

// ByName returns a stable lane color for a ref name.
func ByName(name string) *style.TextStyle {
	return ByIndex(IndexForName(name))
}

// Label returns the filled label style in lane color i (local branches).
func Label(i int) style.TextStyle {
	return labelStyles[wrap(i)]
}

// DimLabel returns a darker filled label style in lane color i (remote branches).
func DimLabel(i int) style.TextStyle {
	return dimLabelStyles[wrap(i)]
}
