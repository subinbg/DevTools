package presentation

import (
	"testing"

	"github.com/gookit/color"
	"github.com/jesseduffield/lazygit/pkg/commands/models"
	"github.com/jesseduffield/lazygit/pkg/i18n"
	"github.com/stretchr/testify/assert"
	"github.com/xo/terminfo"
)

func TestWorkingTreeSummary(t *testing.T) {
	files := []*models.File{
		{Path: "a", Tracked: true, HasStagedChanges: true},
		{Path: "b", Tracked: true, HasStagedChanges: true, HasUnstagedChanges: true},
		{Path: "c", Tracked: true, HasUnstagedChanges: true},
		{Path: "d", Tracked: false, HasUnstagedChanges: true},                         // untracked
		{Path: "e", Tracked: false, HasStagedChanges: true},                           // newly added, staged
		{Path: "f", Tracked: false, HasStagedChanges: true, HasUnstagedChanges: true}, // added then modified
	}

	assert.Equal(t, WorkingTreeCounts{Staged: 4, Unstaged: 3, Untracked: 1}, CountWorkingTreeChanges(files))

	unstaged, staged := StagingSectionSizes(files)
	assert.Equal(t, 4, unstaged)
	assert.Equal(t, 2, staged)

	oldColorLevel := color.ForceSetColorLevel(terminfo.ColorLevelNone)
	defer color.ForceSetColorLevel(oldColorLevel)

	tr := i18n.EnglishTranslationSet()
	assert.Equal(t, "◌ WIP · 4 staged · 3 unstaged · 1 untracked", FormatWorkingTreeRow(files, tr, true))
	assert.Equal(t, "◌ WIP · 1 untracked", FormatWorkingTreeRow(files[3:4], tr, true))
	assert.Equal(t, "◌ WIP", FormatWorkingTreeRow(nil, tr, true))
	assert.Equal(t, "WIP · 1 untracked", FormatWorkingTreeRow(files[3:4], tr, false))
}
