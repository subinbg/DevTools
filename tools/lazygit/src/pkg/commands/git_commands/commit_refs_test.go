package git_commands

import (
	"testing"

	"github.com/jesseduffield/lazygit/pkg/commands/models"
	"github.com/stretchr/testify/assert"
)

func TestParseDecorations(t *testing.T) {
	scenarios := []struct {
		testName      string
		decorations   string
		expectedRefs  []models.CommitRef
		expectedShort string
	}{
		{
			testName:      "empty",
			decorations:   "",
			expectedRefs:  nil,
			expectedShort: "",
		},
		{
			testName:    "checked out branch, remote branch and tag",
			decorations: "HEAD -> refs/heads/main, refs/remotes/origin/main, tag: refs/tags/v1.0.0",
			expectedRefs: []models.CommitRef{
				{Kind: models.CommitRefLocalBranch, Name: "main", IsHead: true},
				{Kind: models.CommitRefRemoteBranch, Name: "origin/main"},
				{Kind: models.CommitRefTag, Name: "v1.0.0"},
			},
			expectedShort: "HEAD -> main, origin/main, tag: v1.0.0",
		},
		{
			testName:    "branch with slashes, and origin/HEAD is skipped but kept in the short form",
			decorations: "refs/heads/feature/foo, refs/remotes/origin/feature/foo, refs/remotes/origin/HEAD",
			expectedRefs: []models.CommitRef{
				{Kind: models.CommitRefLocalBranch, Name: "feature/foo"},
				{Kind: models.CommitRefRemoteBranch, Name: "origin/feature/foo"},
			},
			expectedShort: "feature/foo, origin/feature/foo, origin/HEAD",
		},
		{
			testName:    "detached head and shallow clone marker",
			decorations: "grafted, HEAD, tag: refs/tags/v2",
			expectedRefs: []models.CommitRef{
				{Kind: models.CommitRefDetachedHead, Name: "HEAD", IsHead: true},
				{Kind: models.CommitRefTag, Name: "v2"},
			},
			expectedShort: "grafted, HEAD, tag: v2",
		},
		{
			testName:    "other refs",
			decorations: "refs/stash, refs/bisect/bad",
			expectedRefs: []models.CommitRef{
				{Kind: models.CommitRefOther, Name: "refs/stash"},
				{Kind: models.CommitRefOther, Name: "refs/bisect/bad"},
			},
			expectedShort: "refs/stash, refs/bisect/bad",
		},
		{
			testName:    "short form input round-trips",
			decorations: "HEAD -> master, origin/master, tag: v0.15.2",
			expectedRefs: []models.CommitRef{
				{Kind: models.CommitRefOther, Name: "master", IsHead: true},
				{Kind: models.CommitRefOther, Name: "origin/master"},
				{Kind: models.CommitRefTag, Name: "v0.15.2"},
			},
			expectedShort: "HEAD -> master, origin/master, tag: v0.15.2",
		},
	}

	for _, s := range scenarios {
		t.Run(s.testName, func(t *testing.T) {
			refs, short := parseDecorations(s.decorations)
			assert.Equal(t, s.expectedRefs, refs)
			assert.Equal(t, s.expectedShort, short)
		})
	}
}
