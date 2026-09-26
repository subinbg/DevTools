package helpers

import (
	"fmt"
	"strings"
	"time"

	"github.com/jesseduffield/generics/set"
	"github.com/jesseduffield/lazygit/pkg/commands/git_commands"
	"github.com/jesseduffield/lazygit/pkg/commands/models"
	"github.com/jesseduffield/lazygit/pkg/gui/presentation"
	"github.com/jesseduffield/lazygit/pkg/gui/style"
	"github.com/jesseduffield/lazygit/pkg/gui/types"
	"github.com/jesseduffield/lazygit/pkg/utils"
	"github.com/samber/lo"
)

// LogGraphHelper renders a commit log into the main panel using lazygit's own
// commit list rendering (graph, ref labels, authors, dates), as an alternative
// to showing the output of `git log --graph` (see gui.mainViewCommitGraph).
type LogGraphHelper struct {
	c *HelperCommon
}

func NewLogGraphHelper(c *HelperCommon) *LogGraphHelper {
	return &LogGraphHelper{c: c}
}

// AllBranchesTask returns a task rendering the log of all branches.
func (self *LogGraphHelper) AllBranchesTask() *types.RenderFuncTask {
	return self.task("HEAD", nil, true)
}

// BranchTask returns a task rendering the log of the given local branch.
func (self *LogGraphHelper) BranchTask(branch *models.Branch) *types.RenderFuncTask {
	return self.RefTask(branch, branch)
}

// RefTask returns a task rendering the log of any ref (a remote branch, a
// tag). refForPushedStatus is the local branch whose unpushed commits are
// marked, or nil.
func (self *LogGraphHelper) RefTask(ref models.Ref, refForPushedStatus models.Ref) *types.RenderFuncTask {
	return self.task(ref.FullRefName(), refForPushedStatus, false)
}

// task captures everything the rendering needs from the model here, on the
// UI thread, so that the render goroutine doesn't read the model while it's
// being refreshed.
func (self *LogGraphHelper) task(refName string, refForPushedStatus models.Ref, all bool) *types.RenderFuncTask {
	branches := self.c.Model().Branches
	worktrees := self.c.Model().Worktrees
	checkedOutBranch := self.c.Model().CheckedOutBranch
	mainBranches := self.c.Model().MainBranches
	hashPool := self.c.Model().HashPool
	hasRebaseUpdateRefsConfig := self.c.Git().Config.GetRebaseUpdateRefs()
	userConfig := self.c.UserConfig()

	if all {
		if branch, ok := lo.Find(branches, func(b *models.Branch) bool { return b.Name == checkedOutBranch }); ok {
			refForPushedStatus = branch
		}
	}

	key := fmt.Sprintf("logGraph:%s:%v", refName, all)

	render := func(stop <-chan struct{}) string {
		commits, err := self.c.Git().Loaders.CommitLoader.GetCommits(
			git_commands.GetCommitsOptions{
				Limit:                true,
				IncludeRebaseCommits: false,
				RefName:              refName,
				RefForPushedStatus:   refForPushedStatus,
				All:                  all,
				MainBranches:         mainBranches,
				HashPool:             hashPool,
			},
		)
		if err != nil {
			return style.FgRed.Sprint(err.Error())
		}
		if len(commits) == 0 {
			return self.c.Tr.NoCommitsThisBranch
		}

		select {
		case <-stop:
			return ""
		default:
		}

		displayStrings := presentation.GetCommitListDisplayStrings(
			self.c.Common,
			commits,
			branches,
			worktrees,
			checkedOutBranch,
			hasRebaseUpdateRefsConfig,
			true,
			set.New[string](),
			"",
			"",
			userConfig.Gui.TimeFormat,
			userConfig.Gui.ShortTimeFormat,
			time.Now(),
			userConfig.Git.ParseEmoji,
			nil,
			0,
			len(commits),
			true,
			git_commands.NewNullBisectInfo(),
			presentation.CommitListOpts{},
		)
		lines, _ := utils.RenderDisplayStrings(displayStrings, nil)

		if len(commits) >= git_commands.CommitLimit {
			lines = append(lines, "", style.FgBlue.Sprintf(self.c.Tr.ShowingFirstNCommits, git_commands.CommitLimit))
		}

		return strings.Join(lines, "\n")
	}

	return types.NewRenderFuncTask(key, render)
}
