package presentation

import (
	"testing"

	"github.com/gookit/color"
	"github.com/jesseduffield/lazygit/pkg/commands/models"
	"github.com/samber/lo"
	"github.com/stretchr/testify/assert"
	"github.com/xo/terminfo"
)

func remoteBranch(remote string, name string) *models.RemoteBranch {
	return &models.RemoteBranch{RemoteName: remote, Name: name, CommitHash: "0123456789abcdef", Recency: "2w"}
}

func fullNames(branches []*models.RemoteBranch) []string {
	return lo.Map(branches, func(b *models.RemoteBranch, _ int) string { return b.FullName() })
}

func TestUnfoldRemoteBranches(t *testing.T) {
	remotes := []*models.Remote{
		{Name: "origin", Branches: []*models.RemoteBranch{
			remoteBranch("origin", "main"),
			remoteBranch("origin", "feature/a"),
			remoteBranch("origin", "fix/x"),
			remoteBranch("origin", "feature/b"),
		}},
		{Name: "fork", Branches: []*models.RemoteBranch{
			remoteBranch("fork", "feature/a"),
			remoteBranch("fork", "feature/c"),
		}},
	}

	assert.Equal(t,
		[]string{"origin/main", "origin/feature/a", "origin/feature/b", "origin/fix/x", "fork/feature/a", "fork/feature/c"},
		fullNames(UnfoldRemoteBranches(remotes, true)),
		"grouped per remote, remotes in order")

	assert.Equal(t,
		[]string{"origin/main", "origin/feature/a", "origin/fix/x", "origin/feature/b", "fork/feature/a", "fork/feature/c"},
		fullNames(UnfoldRemoteBranches(remotes, false)))

	assert.Empty(t, UnfoldRemoteBranches(nil, true))
}

func TestRemoteListHeaders(t *testing.T) {
	items := UnfoldRemoteBranches([]*models.Remote{
		{Name: "origin", Branches: []*models.RemoteBranch{
			remoteBranch("origin", "main"),
			remoteBranch("origin", "feature/a"),
			remoteBranch("origin", "feature/b"),
		}},
		{Name: "fork", Branches: []*models.RemoteBranch{
			remoteBranch("fork", "feature/a"),
			remoteBranch("fork", "feature/c"),
		}},
	}, true)

	headers, prefixes := RemoteListHeaders(items, true, true)
	assert.Equal(t, []RemoteListHeader{
		{Index: 0, Remote: "origin"},
		{Index: 1, Prefix: "feature/"},
		{Index: 3, Remote: "fork"},
		{Index: 3, Prefix: "feature/"},
	}, headers)
	assert.Equal(t, map[string]string{
		"origin/feature/a": "feature/", "origin/feature/b": "feature/",
		"fork/feature/a": "feature/", "fork/feature/c": "feature/",
	}, prefixes, "folders are keyed by full name, so equal names on two remotes don't clash")

	headers, prefixes = RemoteListHeaders(items, false, false)
	assert.Empty(t, headers)
	assert.Empty(t, prefixes)

	headers, _ = RemoteListHeaders(items, true, false)
	assert.Equal(t, []RemoteListHeader{{Index: 0, Remote: "origin"}, {Index: 3, Remote: "fork"}}, headers)

	// a filtered subset: a folder left with one branch is no folder any more
	filtered := []*models.RemoteBranch{items[0], items[1], items[3]}
	headers, prefixes = RemoteListHeaders(filtered, true, true)
	assert.Equal(t, []RemoteListHeader{{Index: 0, Remote: "origin"}, {Index: 2, Remote: "fork"}}, headers)
	assert.Empty(t, prefixes)

	headers, _ = RemoteListHeaders(nil, true, true)
	assert.Empty(t, headers)
}

func TestGetRemoteBranchListDisplayStrings(t *testing.T) {
	oldColorLevel := color.ForceSetColorLevel(terminfo.ColorLevelNone)
	defer color.ForceSetColorLevel(oldColorLevel)

	branches := []*models.RemoteBranch{
		remoteBranch("origin", "main"),
		remoteBranch("origin", "feature/a"),
	}
	prefixes := map[string]string{"origin/feature/a": "feature/"}

	assert.Equal(t, [][]string{
		{"2w", "01234567", "  main"},
		{"2w", "01234567", "    a"},
	}, GetRemoteBranchListDisplayStrings(branches, RemoteBranchListOpts{
		GroupPrefixes:  prefixes,
		ShowRecency:    true,
		ShowCommitHash: true,
		Indent:         RefGroupIndent,
	}))

	assert.Equal(t, [][]string{
		{"main"},
		{"  a"},
	}, GetRemoteBranchListDisplayStrings(branches, RemoteBranchListOpts{GroupPrefixes: prefixes}), "upstream's layout: names only")

	assert.Equal(t, 2, RemoteBranchNameColumn(true, true))
	assert.Equal(t, 1, RemoteBranchNameColumn(true, false))
	assert.Equal(t, 0, RemoteBranchNameColumn(false, false))
}
