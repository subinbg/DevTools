package presentation

import (
	"strings"
	"testing"
	"time"

	"github.com/gookit/color"
	"github.com/jesseduffield/generics/set"
	"github.com/jesseduffield/lazygit/pkg/commands/git_commands"
	"github.com/jesseduffield/lazygit/pkg/commands/models"
	"github.com/jesseduffield/lazygit/pkg/common"
	"github.com/jesseduffield/lazygit/pkg/utils"
	"github.com/samber/lo"
	"github.com/stretchr/testify/assert"
	"github.com/xo/terminfo"
)

func TestGetCommitListDisplayStringsRefLabels(t *testing.T) {
	scenarios := []struct {
		testName   string
		commitOpts []models.NewCommitOpts
		worktrees  []*models.Worktree
		showGraph  bool
		expected   string
	}{
		{
			testName: "checked out branch first, then local branches, remote branches, tags",
			commitOpts: []models.NewCommitOpts{
				{Name: "commit1", Hash: "hash1", Refs: []models.CommitRef{
					{Kind: models.CommitRefRemoteBranch, Name: "origin/main"},
					{Kind: models.CommitRefTag, Name: "v1"},
					{Kind: models.CommitRefLocalBranch, Name: "feature"},
					{Kind: models.CommitRefLocalBranch, Name: "main", IsHead: true},
				}},
				{Name: "commit2", Hash: "hash2"},
			},
			expected: formatExpected(`
		hash1  ✓ main   feature   origin/main   v1  commit1
		hash2 commit2
						`),
		},
		{
			testName: "branch checked out in another worktree gets a marker",
			commitOpts: []models.NewCommitOpts{
				{Name: "commit1", Hash: "hash1", Refs: []models.CommitRef{
					{Kind: models.CommitRefLocalBranch, Name: "feature"},
					{Kind: models.CommitRefLocalBranch, Name: "main", IsHead: true},
				}},
			},
			worktrees: []*models.Worktree{
				{Branch: "main", IsCurrent: true},
				{Branch: "feature", IsCurrent: false},
			},
			expected: formatExpected(`
		hash1  ✓ main   ⌂ feature  commit1
						`),
		},
		{
			testName: "more than four refs are summarised",
			commitOpts: []models.NewCommitOpts{
				{Name: "commit1", Hash: "hash1", Refs: lo.Map([]string{"b1", "b2", "b3", "b4", "b5", "b6"},
					func(name string, _ int) models.CommitRef {
						return models.CommitRef{Kind: models.CommitRefLocalBranch, Name: name}
					})},
			},
			expected: formatExpected(`
		hash1  b1   b2   b3   b4  +2 commit1
						`),
		},
		{
			testName: "detached head",
			commitOpts: []models.NewCommitOpts{
				{Name: "commit1", Hash: "hash1", Refs: []models.CommitRef{
					{Kind: models.CommitRefTag, Name: "v1"},
					{Kind: models.CommitRefDetachedHead, Name: "HEAD", IsHead: true},
				}},
			},
			expected: formatExpected(`
		hash1  ✓ HEAD   v1  commit1
						`),
		},
		{
			testName: "labels are shown alongside the graph",
			commitOpts: []models.NewCommitOpts{
				{Name: "commit1", Hash: "hash1", Parents: []string{"hash2"}, Refs: []models.CommitRef{
					{Kind: models.CommitRefLocalBranch, Name: "main", IsHead: true},
				}},
				{Name: "commit2", Hash: "hash2", Parents: []string{"hash3"}, Refs: []models.CommitRef{
					{Kind: models.CommitRefTag, Name: "v1"},
				}},
			},
			showGraph: true,
			expected: formatExpected(`
		hash1 ●  ✓ main  commit1
		hash2 ●  v1  commit2
						`),
		},
	}

	oldColorLevel := color.ForceSetColorLevel(terminfo.ColorLevelNone)
	defer color.ForceSetColorLevel(oldColorLevel)

	common := common.NewDummyCommon()
	assert.True(t, common.UserConfig().Gui.ShowRefLabelsInCommitsView, "ref labels should be on by default")

	for _, s := range scenarios {
		t.Run(s.testName, func(t *testing.T) {
			hashPool := &utils.StringPool{}
			commits := lo.Map(s.commitOpts,
				func(opts models.NewCommitOpts, _ int) *models.Commit { return models.NewCommit(hashPool, opts) })

			result := GetCommitListDisplayStrings(
				common,
				commits,
				nil,
				s.worktrees,
				"main",
				false,
				false,
				set.New[string](),
				"",
				"",
				"",
				"",
				time.Date(2020, 1, 1, 0, 0, 0, 0, time.UTC),
				false,
				nil,
				0,
				len(commits),
				s.showGraph,
				git_commands.NewNullBisectInfo(),
				CommitListOpts{},
			)

			renderedLines, _ := utils.RenderDisplayStrings(result, nil)
			renderedResult := strings.Join(renderedLines, "\n")
			t.Logf("\n%s", renderedResult)

			assert.EqualValues(t, s.expected, renderedResult)
		})
	}
}
