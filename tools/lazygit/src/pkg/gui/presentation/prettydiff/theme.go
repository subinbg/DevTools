// Package prettydiff renders unified diffs the way graphical git clients do:
// with old and new line numbers in a gutter, added and removed lines on tinted
// backgrounds, the words that changed within a line highlighted, and each
// file introduced by a header with its counts of added and removed lines.
//
// It formats both the diffs streamed into the main panel from git (Reader) and
// the patches of the staging and patch-building panels (Renderer, used by the
// patch package), so a diff looks the same wherever it is shown.
package prettydiff

import (
	"strings"

	"github.com/gookit/color"
	"github.com/jesseduffield/lazygit/pkg/gui/style"
)

type rgb struct{ r, g, b uint8 }

// Theme is a set of colors for the diff tints.
type Theme struct {
	Name string

	addBg, addEmphBg, addFg rgb
	delBg, delEmphBg, delFg rgb
	hunkBg, hunkFg          rgb
	fileBg, fileFg          rgb
	gutterFg, metaFg        rgb

	styles themeStyles
}

// the derived text styles, computed once per theme
type themeStyles struct {
	addText, addEmph, addMarker, addIncluded, addGutter    style.TextStyle
	delText, delEmph, delMarker, delIncluded, delGutter    style.TextStyle
	gutter, hunk, file, fileAdded, fileRemoved, meta, noNL style.TextStyle
}

var (
	darkTheme = newTheme(Theme{
		Name:      "dark",
		addBg:     rgb{0x14, 0x30, 0x1f},
		addEmphBg: rgb{0x21, 0x5d, 0x36},
		addFg:     rgb{0x3f, 0xb9, 0x50},
		delBg:     rgb{0x3a, 0x1d, 0x21},
		delEmphBg: rgb{0x74, 0x2c, 0x33},
		delFg:     rgb{0xf8, 0x51, 0x49},
		hunkBg:    rgb{0x1c, 0x28, 0x38},
		hunkFg:    rgb{0x7f, 0xa6, 0xd9},
		fileBg:    rgb{0x2b, 0x2f, 0x36},
		fileFg:    rgb{0xe6, 0xed, 0xf3},
		gutterFg:  rgb{0x6e, 0x76, 0x81},
		metaFg:    rgb{0x8b, 0x94, 0x9e},
	})

	lightTheme = newTheme(Theme{
		Name:      "light",
		addBg:     rgb{0xe6, 0xff, 0xec},
		addEmphBg: rgb{0xab, 0xf2, 0xbc},
		addFg:     rgb{0x1a, 0x7f, 0x37},
		delBg:     rgb{0xff, 0xeb, 0xe9},
		delEmphBg: rgb{0xff, 0xc1, 0xc0},
		delFg:     rgb{0xcf, 0x22, 0x2e},
		hunkBg:    rgb{0xdd, 0xf4, 0xff},
		hunkFg:    rgb{0x09, 0x69, 0xda},
		fileBg:    rgb{0xea, 0xee, 0xf2},
		fileFg:    rgb{0x1f, 0x23, 0x28},
		gutterFg:  rgb{0x6e, 0x77, 0x81},
		metaFg:    rgb{0x65, 0x6d, 0x76},
	})
)

// ThemeByName returns the theme of the given name ("dark" or "light"); any
// other name gives the dark theme.
func ThemeByName(name string) Theme {
	if strings.EqualFold(name, "light") {
		return lightTheme
	}
	return darkTheme
}

// ThemeIf returns the theme of the given name when enabled, and nil
// otherwise: the form the patch formatting takes its options in.
func ThemeIf(enabled bool, name string) *Theme {
	if !enabled {
		return nil
	}
	theme := ThemeByName(name)
	return &theme
}

func toColor(c rgb) style.Color {
	return style.NewRGBColor(color.RGB(c.r, c.g, c.b))
}

func fg(c rgb) style.TextStyle {
	return style.New().SetFg(toColor(c))
}

func bg(c rgb) style.TextStyle {
	return style.New().SetBg(toColor(c))
}

func fgBg(f rgb, b rgb) style.TextStyle {
	return style.New().SetFg(toColor(f)).SetBg(toColor(b))
}

func newTheme(t Theme) Theme {
	t.styles = themeStyles{
		addText:     bg(t.addBg),
		addEmph:     bg(t.addEmphBg),
		addMarker:   fgBg(t.addFg, t.addBg).SetBold(),
		addIncluded: fgBg(t.addBg, t.addFg).SetBold(),
		addGutter:   fgBg(t.addFg, t.addBg),
		delText:     bg(t.delBg),
		delEmph:     bg(t.delEmphBg),
		delMarker:   fgBg(t.delFg, t.delBg).SetBold(),
		delIncluded: fgBg(t.delBg, t.delFg).SetBold(),
		delGutter:   fgBg(t.delFg, t.delBg),
		gutter:      fg(t.gutterFg),
		hunk:        fgBg(t.hunkFg, t.hunkBg),
		file:        fgBg(t.fileFg, t.fileBg).SetBold(),
		fileAdded:   fgBg(t.addFg, t.fileBg),
		fileRemoved: fgBg(t.delFg, t.fileBg),
		meta:        fgBg(t.metaFg, t.fileBg),
		noNL:        fg(t.metaFg),
	}
	return t
}
