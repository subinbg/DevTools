package context

import (
	"github.com/jesseduffield/lazygit/pkg/commands/models"
	"github.com/jesseduffield/lazygit/pkg/gui/presentation"
	"github.com/jesseduffield/lazygit/pkg/gui/types"
	"github.com/samber/lo"
)

type BranchesContext struct {
	*FilteredListViewModel[*models.Branch]
	*ListContextTrait
}

var (
	_ types.IListContext    = (*BranchesContext)(nil)
	_ types.DiffableContext = (*BranchesContext)(nil)
)

func NewBranchesContext(c *ContextCommon) *BranchesContext {
	branchName := func(branch *models.Branch) string { return branch.Name }
	groupingEnabled := func() bool { return c.UserConfig().Gui.GroupBranchesByPrefix }

	viewModel := NewFilteredListViewModel(
		func() []*models.Branch {
			if !groupingEnabled() {
				return c.Model().Branches
			}
			return presentation.GroupByRefNamePrefix(c.Model().Branches, branchName)
		},
		func(branch *models.Branch) []string {
			return []string{branch.Name}
		},
	)

	fullDescription := func() bool { return c.State().GetRepoState().GetScreenMode() != types.SCREEN_NORMAL }

	getDisplayStrings := func(_ int, _ int) [][]string {
		items := viewModel.GetItems()
		var groupPrefixes map[string]string
		if groupingEnabled() {
			groupPrefixes = presentation.RefGroupPrefixes(items, branchName)
		}
		return presentation.GetBranchListDisplayStrings(
			items,
			c.State().GetItemOperation,
			c.Model().PullRequestsMap,
			fullDescription(),
			c.Modes().Diffing.Ref,
			c.Views().Branches.InnerWidth()+c.Views().Branches.OriginX(),
			c.Tr,
			c.UserConfig(),
			c.Model().Worktrees,
			groupPrefixes,
		)
	}

	// Folder headers above groups of branches sharing a name prefix, aligned
	// with the branch name column (recency, PR icon, optional commit hash).
	getNonModelItems := func() []*NonModelItem {
		if !groupingEnabled() {
			return nil
		}
		nameColumn := 2
		if fullDescription() || c.UserConfig().Gui.ShowBranchCommitHash {
			nameColumn = 3
		}
		return lo.Map(presentation.RefGroups(viewModel.GetItems(), branchName),
			func(group presentation.RefGroup, _ int) *NonModelItem {
				return &NonModelItem{
					Index:   group.Index,
					Content: presentation.FormatRefGroupHeader(group.Prefix),
					Column:  nameColumn,
				}
			})
	}

	self := &BranchesContext{
		FilteredListViewModel: viewModel,
		ListContextTrait: &ListContextTrait{
			Context: NewSimpleContext(NewBaseContext(NewBaseContextOpts{
				View:                       c.Views().Branches,
				WindowName:                 "branches",
				Key:                        LOCAL_BRANCHES_CONTEXT_KEY,
				Kind:                       types.SIDE_CONTEXT,
				Focusable:                  true,
				NeedsRerenderOnWidthChange: types.NEEDS_RERENDER_ON_WIDTH_CHANGE_WHEN_WIDTH_CHANGES,
			})),
			ListRenderer: ListRenderer{
				list:              viewModel,
				getDisplayStrings: getDisplayStrings,
				getNonModelItems:  getNonModelItems,
			},
			c: c,
		},
	}

	return self
}

func (self *BranchesContext) GetSelectedRef() models.Ref {
	branch := self.GetSelected()
	if branch == nil {
		return nil
	}
	return branch
}

func (self *BranchesContext) GetDiffTerminals() []string {
	// for our local branches we want to include both the branch and its upstream
	branch := self.GetSelected()
	if branch != nil {
		names := []string{branch.ID()}
		if branch.IsTrackingRemote() {
			names = append(names, branch.ID()+"@{u}")
		}
		return names
	}
	return nil
}

func (self *BranchesContext) RefForAdjustingLineNumberInDiff() string {
	branch := self.GetSelected()
	if branch != nil {
		return branch.ID()
	}
	return ""
}

func (self *BranchesContext) ShowBranchHeadsInSubCommits() bool {
	return true
}
