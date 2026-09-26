package context

import (
	"github.com/jesseduffield/lazygit/pkg/commands/models"
	"github.com/jesseduffield/lazygit/pkg/gui/presentation"
	"github.com/jesseduffield/lazygit/pkg/gui/types"
	"github.com/samber/lo"
)

// RemoteBranchesContext lists remote branches. Upstream, it shows the branches
// of the remote entered from the remotes list and is transient (escape returns
// to that list). With gui.unfoldRemotes it is the Remotes tab itself and lists
// every remote's branches, under a header per remote.
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
	// fixed for the session: the tabs are wired up according to it at startup
	unfolded := c.UserConfig().Gui.UnfoldRemotes
	// the unfolded list has the columns of the local branches list
	showRecency := unfolded
	showCommitHash := func() bool { return unfolded && c.UserConfig().Gui.ShowBranchCommitHash }

	viewModel := NewFilteredListViewModel(
		func() []*models.RemoteBranch {
			if unfolded {
				return presentation.UnfoldRemoteBranches(c.Model().Remotes, groupingEnabled())
			}
			if !groupingEnabled() {
				return c.Model().RemoteBranches
			}
			return presentation.GroupByRefNamePrefix(c.Model().RemoteBranches, branchName)
		},
		func(remoteBranch *models.RemoteBranch) []string {
			return []string{remoteBranch.Name, remoteBranch.FullName()}
		},
	)

	getDisplayStrings := func(_ int, _ int) [][]string {
		items := viewModel.GetItems()
		_, groupPrefixes := presentation.RemoteListHeaders(items, unfolded, groupingEnabled())
		indent := ""
		if unfolded {
			indent = presentation.RefGroupIndent
		}
		return presentation.GetRemoteBranchListDisplayStrings(items, presentation.RemoteBranchListOpts{
			DiffName:       c.Modes().Diffing.Ref,
			GroupPrefixes:  groupPrefixes,
			ShowRecency:    showRecency,
			ShowCommitHash: showCommitHash(),
			Indent:         indent,
		})
	}

	// Headers above each remote's branches (when unfolded) and above groups of
	// branches sharing a name prefix
	getNonModelItems := func() []*NonModelItem {
		headers, _ := presentation.RemoteListHeaders(viewModel.GetItems(), unfolded, groupingEnabled())
		column := presentation.RemoteBranchNameColumn(showRecency, showCommitHash())
		return lo.Map(headers, func(header presentation.RemoteListHeader, _ int) *NonModelItem {
			var content string
			if header.Remote != "" {
				content = presentation.FormatRemoteHeader(header.Remote)
			} else {
				content = presentation.FormatRefGroupHeader(header.Prefix)
				if unfolded {
					content = presentation.RefGroupIndent + content
				}
			}
			return &NonModelItem{
				Index:   header.Index,
				Content: content,
				Column:  column,
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
				Transient:                   !unfolded,
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
