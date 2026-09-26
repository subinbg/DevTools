package context

import (
	"github.com/jesseduffield/lazygit/pkg/commands/models"
	"github.com/jesseduffield/lazygit/pkg/gui/presentation"
	"github.com/jesseduffield/lazygit/pkg/gui/types"
	"github.com/samber/lo"
)

type RemoteBranchesContext struct {
	*FilteredListViewModel[*models.RemoteBranch]
	*ListContextTrait
	*DynamicTitleBuilder
}

var (
	_ types.IListContext    = (*RemoteBranchesContext)(nil)
	_ types.DiffableContext = (*RemoteBranchesContext)(nil)
)

func NewRemoteBranchesContext(
	c *ContextCommon,
) *RemoteBranchesContext {
	branchName := func(branch *models.RemoteBranch) string { return branch.Name }
	groupingEnabled := func() bool { return c.UserConfig().Gui.GroupBranchesByPrefix }

	viewModel := NewFilteredListViewModel(
		func() []*models.RemoteBranch {
			if !groupingEnabled() {
				return c.Model().RemoteBranches
			}
			return presentation.GroupByRefNamePrefix(c.Model().RemoteBranches, branchName)
		},
		func(remoteBranch *models.RemoteBranch) []string {
			return []string{remoteBranch.Name}
		},
	)

	getDisplayStrings := func(_ int, _ int) [][]string {
		items := viewModel.GetItems()
		var groupPrefixes map[string]string
		if groupingEnabled() {
			groupPrefixes = presentation.RefGroupPrefixes(items, branchName)
		}
		return presentation.GetRemoteBranchListDisplayStrings(items, c.Modes().Diffing.Ref, groupPrefixes)
	}

	// Folder headers above groups of branches sharing a name prefix
	getNonModelItems := func() []*NonModelItem {
		if !groupingEnabled() {
			return nil
		}
		return lo.Map(presentation.RefGroups(viewModel.GetItems(), branchName),
			func(group presentation.RefGroup, _ int) *NonModelItem {
				return &NonModelItem{
					Index:   group.Index,
					Content: presentation.FormatRefGroupHeader(group.Prefix),
					Column:  0,
				}
			})
	}

	return &RemoteBranchesContext{
		FilteredListViewModel: viewModel,
		DynamicTitleBuilder:   NewDynamicTitleBuilder(c.Tr.RemoteBranchesDynamicTitle),
		ListContextTrait: &ListContextTrait{
			Context: NewSimpleContext(NewBaseContext(NewBaseContextOpts{
				View:                        c.Views().RemoteBranches,
				WindowName:                  "branches",
				Key:                         REMOTE_BRANCHES_CONTEXT_KEY,
				Kind:                        types.SIDE_CONTEXT,
				Focusable:                   true,
				Transient:                   true,
				NeedsRerenderOnHeightChange: true,
			})),
			ListRenderer: ListRenderer{
				list:              viewModel,
				getDisplayStrings: getDisplayStrings,
				getNonModelItems:  getNonModelItems,
			},
			c: c,
		},
	}
}

func (self *RemoteBranchesContext) GetSelectedRef() models.Ref {
	remoteBranch := self.GetSelected()
	if remoteBranch == nil {
		return nil
	}
	return remoteBranch
}

func (self *RemoteBranchesContext) GetSelectedRefs() ([]models.Ref, int, int) {
	items, startIdx, endIdx := self.GetSelectedItems()

	refs := lo.Map(items, func(item *models.RemoteBranch, _ int) models.Ref {
		return item
	})

	return refs, startIdx, endIdx
}

func (self *RemoteBranchesContext) GetDiffTerminals() []string {
	itemId := self.GetSelectedItemId()

	return []string{itemId}
}

func (self *RemoteBranchesContext) RefForAdjustingLineNumberInDiff() string {
	return self.GetSelectedItemId()
}

func (self *RemoteBranchesContext) ShowBranchHeadsInSubCommits() bool {
	return true
}
