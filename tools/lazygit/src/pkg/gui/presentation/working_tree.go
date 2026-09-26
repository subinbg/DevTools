package presentation

import (
	"strconv"
	"strings"

	"github.com/jesseduffield/lazygit/pkg/commands/models"
	"github.com/jesseduffield/lazygit/pkg/gui/style"
	"github.com/jesseduffield/lazygit/pkg/i18n"
	"github.com/jesseduffield/lazygit/pkg/theme"
)

// WorkingTreeCounts summarises the working tree: files with staged changes,
// files with unstaged changes (a file can be both), and untracked files.
type WorkingTreeCounts struct {
	Staged    int
	Unstaged  int
	Untracked int
}

func CountWorkingTreeChanges(files []*models.File) WorkingTreeCounts {
	counts := WorkingTreeCounts{}
	for _, file := range files {
		if !file.Tracked && !file.HasStagedChanges {
			counts.Untracked++
			continue
		}
		if file.HasStagedChanges {
			counts.Staged++
		}
		if file.HasUnstagedChanges {
			counts.Unstaged++
		}
	}
	return counts
}

// StagingSectionSizes returns how many of the files belong to the "unstaged"
// section of the files view (files with unstaged changes, including untracked
// ones) and how many to the "staged" section (files whose changes are all
// staged). The flat files view lists them in that order.
func StagingSectionSizes(files []*models.File) (unstaged int, staged int) {
	for _, file := range files {
		if InUnstagedSection(file) {
			unstaged++
		} else {
			staged++
		}
	}
	return unstaged, staged
}

func InUnstagedSection(file *models.File) bool {
	return file.HasUnstagedChanges || !file.HasStagedChanges
}

// FormatWorkingTreeRow renders the "WIP" row shown above the commits when the
// working tree has changes, like GitKraken's work-in-progress node:
// "◌ WIP · 1 staged · 2 unstaged · 1 untracked" (zero counts are omitted).
// withMarker prepends the "◌" node glyph; pass false when the graph draws the
// node itself (see WorkingTreeGraphRow).
func FormatWorkingTreeRow(files []*models.File, tr *i18n.TranslationSet, withMarker bool) string {
	counts := CountWorkingTreeChanges(files)
	parts := []string{}
	if counts.Staged > 0 {
		parts = append(parts, style.FgGreen.Sprint(strconv.Itoa(counts.Staged)+" "+tr.LcStaged))
	}
	if counts.Unstaged > 0 {
		parts = append(parts, theme.UnstagedChangesColor.Sprint(strconv.Itoa(counts.Unstaged)+" "+tr.LcUnstaged))
	}
	if counts.Untracked > 0 {
		parts = append(parts, style.FgMagenta.Sprint(strconv.Itoa(counts.Untracked)+" "+tr.LcUntracked))
	}

	label := tr.WorkInProgress
	if withMarker {
		label = "◌ " + label
	}
	label = theme.DefaultTextColor.SetBold().Sprint(label)
	if len(parts) == 0 {
		return label
	}
	separator := theme.DefaultTextColor.Sprint(" · ")
	return label + separator + strings.Join(parts, separator)
}
