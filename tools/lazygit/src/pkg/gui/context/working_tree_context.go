package context

import (
	"fmt"

	"github.com/jesseduffield/lazygit/pkg/commands/models"
	"github.com/jesseduffield/lazygit/pkg/gui/filetree"
	"github.com/jesseduffield/lazygit/pkg/gui/presentation"
	"github.com/jesseduffield/lazygit/pkg/gui/presentation/icons"
	"github.com/jesseduffield/lazygit/pkg/gui/types"
	"github.com/samber/lo"
)

type WorkingTreeContext struct {
	*filetree.FileTreeViewModel
	*ListContextTrait
}

var (
	_ types.IListContext       = (*WorkingTreeContext)(nil)
	_ types.IFilterableContext = (*WorkingTreeContext)(nil)
)

func NewWorkingTreeContext(c *ContextCommon) *WorkingTreeContext {
	viewModel := filetree.NewFileTreeViewModel(
		func() []*models.File { return c.Model().Files },
		c.Common,
		c.UserConfig().Gui.ShowFileTree,
	)

	getDisplayStrings := func(_ int, _ int) [][]string {
		showFileIcons := icons.IsIconEnabled() && c.UserConfig().Gui.ShowFileIcons
		showNumstat := c.UserConfig().Gui.ShowNumstatInFilesView
		lines := presentation.RenderFileTree(viewModel, c.Model().Submodules, showFileIcons, showNumstat, &c.UserConfig().Gui.CustomIcons, c.UserConfig().Gui.ShowRootItemInFileTree)
		return lo.Map(lines, func(line string, _ int) []string {
			return []string{line}
		})
	}

	// "Unstaged files (n)" / "Staged files (m)" headers, in the flat view only
	// (a tree can't be split by status), and only when no status filter is on.
	getNonModelItems := func() []*NonModelItem {
		if !c.UserConfig().Gui.ShowStagingSectionsInFilesView ||
			viewModel.InTreeMode() ||
			viewModel.GetStatusFilter() != filetree.DisplayAll {
			return nil
		}
		items := viewModel.GetAllItems()
		if len(items) == 0 {
			return nil
		}
		files := lo.FilterMap(items, func(item *filetree.FileNode, _ int) (*models.File, bool) {
			return item.File, item.File != nil
		})
		unstaged, staged := presentation.StagingSectionSizes(files)
		return []*NonModelItem{
			{Index: 0, Content: formatListSectionHeader(fmt.Sprintf("%s (%d)", c.Tr.UnstagedFilesSectionHeader, unstaged))},
			{Index: unstaged, Content: formatListSectionHeader(fmt.Sprintf("%s (%d)", c.Tr.StagedFilesSectionHeader, staged))},
		}
	}

	ctx := &WorkingTreeContext{
		FileTreeViewModel: viewModel,
		ListContextTrait: &ListContextTrait{
			Context: NewSimpleContext(NewBaseContext(NewBaseContextOpts{
				View:       c.Views().Files,
				WindowName: "files",
				Key:        FILES_CONTEXT_KEY,
				Kind:       types.SIDE_CONTEXT,
				Focusable:  true,
			})),
			ListRenderer: ListRenderer{
				list:              viewModel,
				getDisplayStrings: getDisplayStrings,
				getNonModelItems:  getNonModelItems,
			},
			c: c,
		},
	}

	return ctx
}
