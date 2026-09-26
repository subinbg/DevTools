// Package palette provides a fixed set of N distinct colors used to color
// commit-graph lanes (cyclically by lane position) and branch/ref names
// (stably by name hash, so a branch keeps its color across renders; with
// more branches than colors, some branches share a color).
package palette

import (
	"hash/fnv"
	"strings"

	"github.com/gookit/color"
	"github.com/jesseduffield/lazygit/pkg/gui/style"
)

func rgb(r, g, b uint8) style.TextStyle {
	return style.New().SetFg(style.NewRGBColor(color.RGB(r, g, b)))
}

// Colors is the predefined palette (N = 10), ordered so that adjacent
// indices (= adjacent graph lanes) contrast strongly.
var Colors = []style.TextStyle{
	rgb(0x61, 0xaf, 0xef), // blue
	rgb(0x98, 0xc3, 0x79), // green
	rgb(0xe0, 0x6c, 0x75), // red
	rgb(0xe5, 0xc0, 0x7b), // yellow
	rgb(0xc6, 0x78, 0xdd), // purple
	rgb(0x56, 0xb6, 0xc2), // cyan
	rgb(0xff, 0x9e, 0x64), // orange
	rgb(0xf4, 0x7f, 0xd4), // pink
	rgb(0x1a, 0xbc, 0x9c), // teal
	rgb(0x9d, 0x7c, 0xd8), // violet
}

// ByIndex returns the palette color for an index, wrapping cyclically.
func ByIndex(i int) *style.TextStyle {
	n := len(Colors)
	s := Colors[((i%n)+n)%n]
	return &s
}

// ByName returns a stable palette color for a ref name. The remote prefix
// "origin/" is stripped so a branch and its origin counterpart share a color.
func ByName(name string) *style.TextStyle {
	name = strings.TrimPrefix(name, "origin/")
	h := fnv.New32a()
	_, _ = h.Write([]byte(name))
	return ByIndex(int(h.Sum32() % uint32(len(Colors))))
}
