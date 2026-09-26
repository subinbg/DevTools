package patch

import (
	"strings"

	"github.com/jesseduffield/generics/set"
	"github.com/jesseduffield/lazygit/pkg/gui/presentation/prettydiff"
	"github.com/jesseduffield/lazygit/pkg/gui/style"
	"github.com/jesseduffield/lazygit/pkg/theme"
	"github.com/samber/lo"
)

type patchPresenter struct {
	patch *Patch
	// if true, all following fields are ignored
	plain bool

	// line indices for tagged lines (e.g. lines added to a custom patch)
	incLineIndices *set.Set[int]

	// pretty layout (line numbers, tints, word highlights) in this theme; nil
	// for the classic layout
	pretty *prettydiff.Theme
	// pretty layout without colors
	plainText bool
}

// formats the patch as a plain string
func formatPlain(patch *Patch) string {
	presenter := &patchPresenter{
		patch:          patch,
		plain:          true,
		incLineIndices: set.New[int](),
	}
	return presenter.format()
}

func formatRangePlain(patch *Patch, startIdx int, endIdx int) string {
	lines := patch.Lines()[startIdx : endIdx+1]
	return strings.Join(
		lo.Map(lines, func(line *PatchLine, _ int) string {
			return line.Content + "\n"
		}),
		"",
	)
}

type FormatViewOpts struct {
	// line indices for tagged lines (e.g. lines added to a custom patch)
	IncLineIndices *set.Set[int]
	// Pretty renders the patch with the prettydiff layout (old and new line
	// numbers, tinted added and removed lines, highlighted changed words) in
	// the given theme; nil renders the classic layout. Either way each patch
	// line renders as exactly one line.
	Pretty *prettydiff.Theme
	// PlainText renders the pretty layout without colors, e.g. to measure how
	// the lines wrap. Ignored for the classic layout.
	PlainText bool
}

// formats the patch for rendering within a view, meaning it's coloured and
// highlights selected items
func formatView(patch *Patch, opts FormatViewOpts) string {
	includedLineIndices := opts.IncLineIndices
	if includedLineIndices == nil {
		includedLineIndices = set.New[int]()
	}
	presenter := &patchPresenter{
		patch:          patch,
		plain:          false,
		incLineIndices: includedLineIndices,
		pretty:         opts.Pretty,
		plainText:      opts.PlainText,
	}
	return presenter.format()
}

func (self *patchPresenter) format() string {
	// if we have no changes in our patch (i.e. no additions or deletions) then
	// the patch is effectively empty and we can return an empty string
	if !self.patch.ContainsChanges() {
		return ""
	}

	if self.pretty != nil && !self.plain {
		return self.formatPretty()
	}

	stringBuilder := &strings.Builder{}
	lineIdx := 0
	appendLine := func(line string) {
		_, _ = stringBuilder.WriteString(line + "\n")

		lineIdx++
	}

	for _, line := range self.patch.header {
		// always passing false for 'included' here because header lines are not part of the patch
		appendLine(self.formatLineAux(line, theme.DefaultTextColor.SetBold(), false))
	}

	for _, hunk := range self.patch.hunks {
		appendLine(
			self.formatLineAux(
				hunk.formatHeaderStart(),
				style.FgCyan,
				false,
			) +
				// we're splitting the line into two parts: the diff header and the context
				// We explicitly pass 'included' as false for both because these are not part
				// of the actual patch
				self.formatLineAux(
					hunk.headerContext,
					theme.DefaultTextColor,
					false,
				),
		)

		for _, line := range hunk.bodyLines {
			style := self.patchLineStyle(line)
			if line.IsChange() {
				appendLine(self.formatLine(line.Content, style, lineIdx))
			} else {
				appendLine(self.formatLineAux(line.Content, style, false))
			}
		}
	}

	return stringBuilder.String()
}

// formatPretty renders the patch in the prettydiff layout, one line per patch
// line so that the line indices stay those of the patch.
func (self *patchPresenter) formatPretty() string {
	maxLine, added, removed := 0, 0, 0
	for _, hunk := range self.patch.hunks {
		maxLine = max(maxLine, hunk.oldStart+hunk.oldLength(), hunk.newStart+hunk.newLength())
		for _, line := range hunk.bodyLines {
			switch line.Kind {
			case ADDITION:
				added++
			case DELETION:
				removed++
			}
		}
	}
	renderer := prettydiff.NewRenderer(*self.pretty, maxLine, !self.plainText)

	stringBuilder := &strings.Builder{}
	lineIdx := 0
	appendLine := func(line string) {
		_, _ = stringBuilder.WriteString(line + "\n")

		lineIdx++
	}
	text := func(line *PatchLine) string {
		if line.Content == "" {
			return ""
		}
		return line.Content[1:]
	}

	if len(self.patch.header) > 0 {
		path, _ := prettydiff.ParseExtendedHeader(self.patch.header[0], self.patch.header[1:])
		appendLine(renderer.FileHeader(path, added, removed))
		for _, line := range self.patch.header[1:] {
			appendLine(renderer.Meta(line))
		}
	}

	for _, hunk := range self.patch.hunks {
		appendLine(renderer.HunkHeader(hunk.formatHeaderLine()))
		oldNo, newNo := hunk.oldStart, hunk.newStart
		lines := hunk.bodyLines
		for i := 0; i < len(lines); {
			switch lines[i].Kind {
			case ADDITION, DELETION:
				// a block of removed lines followed by added lines (possibly
				// with a "no newline" marker in between) is compared word by
				// word
				j := i
				var dels, adds []string
				for j < len(lines) && lines[j].Kind != CONTEXT {
					switch lines[j].Kind {
					case DELETION:
						dels = append(dels, text(lines[j]))
					case ADDITION:
						adds = append(adds, text(lines[j]))
					}
					j++
				}
				delSpans, addSpans := prettydiff.BlockSpans(dels, adds)
				di, ai := 0, 0
				for _, line := range lines[i:j] {
					included := self.incLineIndices.Includes(lineIdx)
					switch line.Kind {
					case DELETION:
						appendLine(renderer.Del(oldNo, text(line), delSpans[di], included))
						di++
						oldNo++
					case ADDITION:
						appendLine(renderer.Add(newNo, text(line), addSpans[ai], included))
						ai++
						newNo++
					default:
						appendLine(renderer.NoNewline())
					}
				}
				i = j
			case NEWLINE_MESSAGE:
				appendLine(renderer.NoNewline())
				i++
			default:
				appendLine(renderer.Context(oldNo, newNo, text(lines[i])))
				oldNo++
				newNo++
				i++
			}
		}
	}

	return stringBuilder.String()
}

func (self *patchPresenter) patchLineStyle(patchLine *PatchLine) style.TextStyle {
	switch patchLine.Kind {
	case ADDITION:
		return style.FgGreen
	case DELETION:
		return style.FgRed
	default:
		return theme.DefaultTextColor
	}
}

func (self *patchPresenter) formatLine(str string, textStyle style.TextStyle, index int) string {
	included := self.incLineIndices.Includes(index)

	return self.formatLineAux(str, textStyle, included)
}

// 'selected' means you've got it highlighted with your cursor
// 'included' means the line has been included in the patch (only applicable when
// building a patch)
func (self *patchPresenter) formatLineAux(str string, textStyle style.TextStyle, included bool) string {
	if self.plain {
		return str
	}

	firstCharStyle := textStyle
	if included {
		firstCharStyle = firstCharStyle.MergeStyle(style.BgGreen)
	}

	if len(str) < 2 {
		return firstCharStyle.Sprint(str)
	}

	return firstCharStyle.Sprint(str[:1]) + textStyle.Sprint(str[1:])
}
