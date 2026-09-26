package controllers

import (
	"fmt"
	"strings"

	"github.com/jesseduffield/lazygit/pkg/commands/models"
	"github.com/jesseduffield/lazygit/pkg/gocui"
	"github.com/jesseduffield/lazygit/pkg/gui/context"
	"github.com/jesseduffield/lazygit/pkg/gui/types"
	"github.com/jesseduffield/lazygit/pkg/utils"
	"github.com/samber/lo"
)

type RemoteBranchesController struct {
	baseController
	*ListControllerTrait[*models.RemoteBranch]
	c *ControllerCommon

	// for the remote-level actions offered when the remotes are unfolded (see
	// gui.unfoldRemotes), where the list of remotes isn't shown
	remotesController *RemotesController
}

var _ types.IController = &RemoteBranchesController{}

func NewRemoteBranchesController(
	c *ControllerCommon,
	remotesController *RemotesController,
) *RemoteBranchesController {
	return &RemoteBranchesController{
		baseController: baseController{},
		ListControllerTrait: NewListControllerTrait(
			c,
			c.Contexts().RemoteBranches,
			c.Contexts().RemoteBranches.GetSelected,
			c.Contexts().RemoteBranches.GetSelectedItems,
		),
		c:                 c,
		remotesController: remotesController,
	}
}

func (self *RemoteBranchesController) GetKeybindings(opts types.KeybindingsOpts) []*types.Binding {
	bindings := self.getBindings(opts)
	if self.c.UserConfig().Gui.UnfoldRemotes {
		bindings = append(bindings,
			&types.Binding{
				Keys:              opts.GetKeys(opts.Config.Branches.FetchRemote),
				Handler:           self.withItem(self.fetchRemote),
				GetDisabledReason: self.require(self.singleItemSelected()),
				Description:       self.c.Tr.Fetch,
				DisplayOnScreen:   true,
			},
			&types.Binding{
				Keys:        opts.GetKeys(opts.Config.Universal.Edit),
				Handler:     self.remoteOptionsMenu,
				Description: self.c.Tr.ViewRemoteOptions,
				OpensMenu:   true,
			},
		)
	}
	return bindings
}

func (self *RemoteBranchesController) getBindings(opts types.KeybindingsOpts) []*types.Binding {
	return []*types.Binding{
		{
			Keys:              opts.GetKeys(opts.Config.Universal.Select),
			Handler:           self.withItem(self.checkoutBranch),
			GetDisabledReason: self.require(self.singleItemSelected()),
			Description:       self.c.Tr.Checkout,
			Tooltip:           self.c.Tr.RemoteBranchCheckoutTooltip,
			DisplayOnScreen:   true,
		},
		{
			Keys:              opts.GetKeys(opts.Config.Universal.New),
			Handler:           self.withItem(self.newLocalBranch),
			GetDisabledReason: self.require(self.singleItemSelected()),
			Description:       self.c.Tr.NewBranch,
		},
		{
			Keys:        opts.GetKeys(opts.Config.Universal.NewWorktree),
			Handler:     self.withItem(self.c.Helpers().Worktree.NewWorktreeMenuForRemoteBranch),
			Description: self.c.Tr.NewWorktree,
			OpensMenu:   true,
		},
		{
			Keys:              opts.GetKeys(opts.Config.Branches.MergeIntoCurrentBranch),
			Handler:           opts.Guards.OutsideFilterMode(self.withItem(self.merge)),
			GetDisabledReason: self.require(self.singleItemSelected()),
			Description:       self.c.Tr.Merge,
			Tooltip:           self.c.Tr.MergeBranchTooltip,
			DisplayOnScreen:   true,
		},
		{
			Keys:              opts.GetKeys(opts.Config.Branches.RebaseBranch),
			Handler:           opts.Guards.OutsideFilterMode(self.withItem(self.rebase)),
			GetDisabledReason: self.require(self.singleItemSelected()),
			Description:       self.c.Tr.RebaseBranch,
			Tooltip:           self.c.Tr.RebaseBranchTooltip,
			DisplayOnScreen:   true,
		},
		{
			Keys:              opts.GetKeys(opts.Config.Universal.Remove),
			Handler:           self.withItems(self.delete),
			GetDisabledReason: self.require(self.itemRangeSelected()),
			Description:       self.c.Tr.Delete,
			Tooltip:           self.c.Tr.DeleteRemoteBranchTooltip,
			DisplayOnScreen:   true,
		},
		{
			Keys:              opts.GetKeys(opts.Config.Branches.SetUpstream),
			Handler:           self.withItem(self.setAsUpstream),
			GetDisabledReason: self.require(self.singleItemSelected()),
			Description:       self.c.Tr.SetAsUpstream,
			Tooltip:           self.c.Tr.SetAsUpstreamTooltip,
			DisplayOnScreen:   true,
		},
		{
			Keys:        opts.GetKeys(opts.Config.Branches.SortOrder),
			Handler:     self.createSortMenu,
			Description: self.c.Tr.SortOrder,
			OpensMenu:   true,
		},
		{
			Keys:              opts.GetKeys(opts.Config.Commits.ViewResetOptions),
			Handler:           self.withItem(self.createResetMenu),
			GetDisabledReason: self.require(self.singleItemSelected()),
			Description:       self.c.Tr.ViewResetOptions,
			Tooltip:           self.c.Tr.ResetTooltip,
			OpensMenu:         true,
		},
		{
			Keys: opts.GetKeys(opts.Config.Universal.OpenDiffTool),
			Handler: self.withItem(func(selectedBranch *models.RemoteBranch) error {
				return self.c.Helpers().Diff.OpenDiffToolForRef(selectedBranch)
			}),
			GetDisabledReason: self.require(self.singleItemSelected()),
			Description:       self.c.Tr.OpenDiffTool,
		},
	}
}

func (self *RemoteBranchesController) GetOnRenderToMain() func() {
	return func() {
		self.c.Helpers().Diff.WithDiffModeCheck(func() {
			var task types.UpdateTask
			remoteBranch := self.context().GetSelected()
			if remoteBranch == nil {
				task = types.NewRenderStringTask("No branches for this remote")
			} else if self.c.UserConfig().Gui.MainViewCommitGraph {
				task = self.c.Helpers().LogGraph.RefTask(remoteBranch, nil)
			} else {
				cmdObj := self.c.Git().Branch.GetGraphCmdObj(remoteBranch.FullRefName())
				task = types.NewRunCommandTask(cmdObj.GetCmd())
			}

			self.c.RenderToMainViews(types.RefreshMainOpts{
				Pair: self.c.MainViewPairs().Normal,
				Main: &types.ViewUpdateOpts{
					Title: "Remote Branch",
					Task:  task,
				},
			})
		})
	}
}

func (self *RemoteBranchesController) context() *context.RemoteBranchesContext {
	return self.c.Contexts().RemoteBranches
}

func (self *RemoteBranchesController) remoteOf(branch *models.RemoteBranch) *models.Remote {
	remote, _ := lo.Find(self.c.Model().Remotes, func(remote *models.Remote) bool {
		return remote.Name == branch.RemoteName
	})
	return remote
}

// fetchRemote fetches the remote the selected branch belongs to.
func (self *RemoteBranchesController) fetchRemote(branch *models.RemoteBranch) error {
	remote := self.remoteOf(branch)
	if remote == nil {
		return nil
	}
	return self.c.WithWaitingStatus(self.c.Tr.FetchingStatus, func(task gocui.Task) error {
		if err := self.c.Git().Sync.FetchRemote(task, remote.Name); err != nil {
			return err
		}
		self.c.Refresh(types.RefreshOptions{Scope: []types.RefreshableView{types.BRANCHES, types.REMOTES}})
		return nil
	})
}

// remoteOptionsMenu offers the actions of the remotes list (which isn't shown
// when the remotes are unfolded) for the selected branch's remote.
func (self *RemoteBranchesController) remoteOptionsMenu() error {
	var remote *models.Remote
	branch := self.context().GetSelected()
	if branch != nil {
		remote = self.remoteOf(branch)
	}

	items := []*types.MenuItem{}
	if remote != nil {
		items = append(items,
			&types.MenuItem{
				Label:   fmt.Sprintf(self.c.Tr.FetchRemoteItem, remote.Name),
				OnPress: func() error { return self.fetchRemote(branch) },
				Keys:    []gocui.Key{gocui.NewKeyRune('f')},
			},
			&types.MenuItem{
				Label:   fmt.Sprintf(self.c.Tr.EditRemoteItem, remote.Name),
				OnPress: func() error { return self.remotesController.edit(remote) },
				Keys:    []gocui.Key{gocui.NewKeyRune('e')},
			},
			&types.MenuItem{
				Label:   fmt.Sprintf(self.c.Tr.RemoveRemoteItem, remote.Name),
				OnPress: func() error { return self.remotesController.remove(remote) },
				Keys:    []gocui.Key{gocui.NewKeyRune('d')},
			},
		)
	}
	items = append(items,
		&types.MenuItem{
			Label:   self.c.Tr.NewRemote,
			OnPress: self.remotesController.add,
			Keys:    []gocui.Key{gocui.NewKeyRune('n')},
		},
		&types.MenuItem{
			Label:   self.c.Tr.AddForkRemote,
			OnPress: self.remotesController.addFork,
			Keys:    []gocui.Key{gocui.NewKeyRune('a')},
		},
	)

	return self.c.Menu(types.CreateMenuOptions{Title: self.c.Tr.RemoteOptionsTitle, Items: items})
}

func (self *RemoteBranchesController) delete(selectedBranches []*models.RemoteBranch) error {
	return self.c.Helpers().BranchesHelper.ConfirmDeleteRemote(selectedBranches, true)
}

func (self *RemoteBranchesController) merge(selectedBranch *models.RemoteBranch) error {
	return self.c.Helpers().MergeAndRebase.MergeRefIntoCheckedOutBranch(selectedBranch.FullName())
}

func (self *RemoteBranchesController) rebase(selectedBranch *models.RemoteBranch) error {
	return self.c.Helpers().MergeAndRebase.RebaseOntoRef(selectedBranch.FullName())
}

func (self *RemoteBranchesController) createSortMenu() error {
	return self.c.Helpers().Refs.CreateSortOrderMenu(
		[]string{"alphabetical", "date"},
		self.c.Tr.SortOrderPromptRemoteBranches,
		func(sortOrder string) error {
			if self.c.UserConfig().Git.RemoteBranchSortOrder != sortOrder {
				self.c.UserConfig().Git.RemoteBranchSortOrder = sortOrder
				self.c.Contexts().RemoteBranches.SetSelection(0)
				self.c.Refresh(types.RefreshOptions{Scope: []types.RefreshableView{types.REMOTES}})
			}
			return nil
		},
		self.c.UserConfig().Git.RemoteBranchSortOrder)
}

func (self *RemoteBranchesController) createResetMenu(selectedBranch *models.RemoteBranch) error {
	return self.c.Helpers().Refs.CreateGitResetMenu(selectedBranch.FullName(), selectedBranch.FullRefName())
}

func (self *RemoteBranchesController) setAsUpstream(selectedBranch *models.RemoteBranch) error {
	checkedOutBranch := self.c.Helpers().Refs.GetCheckedOutRef()

	message := utils.ResolvePlaceholderString(
		self.c.Tr.SetUpstreamMessage,
		map[string]string{
			"checkedOut": checkedOutBranch.Name,
			"selected":   selectedBranch.FullName(),
		},
	)

	self.c.Confirm(types.ConfirmOpts{
		Title:  self.c.Tr.SetUpstreamTitle,
		Prompt: message,
		HandleConfirm: func() error {
			self.c.LogAction(self.c.Tr.Actions.SetBranchUpstream)
			if err := self.c.Git().Branch.SetUpstream(selectedBranch.RemoteName, selectedBranch.Name, checkedOutBranch.Name); err != nil {
				return err
			}

			self.c.Refresh(types.RefreshOptions{Scope: []types.RefreshableView{types.BRANCHES, types.REMOTES}})
			return nil
		},
	})

	return nil
}

func (self *RemoteBranchesController) newLocalBranch(selectedBranch *models.RemoteBranch) error {
	// will set to the remote's branch name without the remote name
	nameSuggestion := strings.SplitAfterN(selectedBranch.RefName(), "/", 2)[1]

	return self.c.Helpers().Refs.NewBranch(selectedBranch.RefName(), selectedBranch.RefName(), nameSuggestion)
}

func (self *RemoteBranchesController) checkoutBranch(selectedBranch *models.RemoteBranch) error {
	return self.c.Helpers().Refs.CheckoutRemoteBranch(selectedBranch.FullName(), selectedBranch.Name)
}
