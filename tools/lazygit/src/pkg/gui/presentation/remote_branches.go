package presentation

import (
	"github.com/jesseduffield/lazygit/pkg/commands/models"
	"github.com/jesseduffield/lazygit/pkg/gui/presentation/icons"
	"github.com/jesseduffield/lazygit/pkg/gui/style"
	"github.com/jesseduffield/lazygit/pkg/theme"
	"github.com/jesseduffield/lazygit/pkg/utils"
	"github.com/samber/lo"
)

// RemoteBranchListOpts controls how the rows of a remote branches list are
// rendered.
type RemoteBranchListOpts struct {
	DiffName string
	// The folder each branch is shown under, by full name ("origin/feature/x");
	// see RemoteListHeaders.
	GroupPrefixes map[string]string
	// Show the age of the last commit and its hash, like the local branches
	// list does. Upstream's list shows names only.
	ShowRecency    bool
	ShowCommitHash bool
	// Prepended to every name, e.g. when the branches sit under a remote header.
	Indent string
}

// GetRemoteBranchListDisplayStrings renders the rows of a remote branches
// list, with the same columns as the local branches list: recency, commit
// hash, (icon,) name.
func GetRemoteBranchListDisplayStrings(branches []*models.RemoteBranch, opts RemoteBranchListOpts) [][]string {
	return lo.Map(branches, func(branch *models.RemoteBranch, _ int) []string {
		return getRemoteBranchDisplayStrings(branch, opts)
	})
}

func getRemoteBranchDisplayStrings(b *models.RemoteBranch, opts RemoteBranchListOpts) []string {
	textStyle := GetRemoteBranchTextStyle(b)
	if b.FullName() == opts.DiffName {
		textStyle = theme.DiffTerminalColor
	}

	res := make([]string, 0, 4)
	if opts.ShowRecency {
		res = append(res, style.FgCyan.Sprint(b.Recency))
	}
	if opts.ShowCommitHash {
		res = append(res, utils.ShortHash(b.CommitHash))
	}
	if icons.IsIconEnabled() {
		res = append(res, textStyle.Sprint(icons.IconForRemoteBranch(b)))
	}
	res = append(res, textStyle.Sprint(opts.Indent+refGroupDisplayName(b.Name, opts.GroupPrefixes[b.FullName()])))
	return res
}

// RemoteBranchNameColumn is the index of the name column in the rows of
// GetRemoteBranchListDisplayStrings, which the remote and folder headers are
// aligned with.
func RemoteBranchNameColumn(showRecency bool, showCommitHash bool) int {
	column := 0
	if showRecency {
		column++
	}
	if showCommitHash {
		column++
	}
	if icons.IsIconEnabled() {
		column++
	}
	return column
}

// FormatRemoteHeader renders the header shown above a remote's branches in the
// unfolded remotes list.
func FormatRemoteHeader(name string) string {
	marker := "▾"
	if icons.IsIconEnabled() {
		marker = icons.DEFAULT_DIRECTORY_ICON.Icon
	}
	return theme.DefaultTextColor.SetBold().Sprint(marker + " " + name)
}

// UnfoldRemoteBranches lists every remote's branches one after the other, in
// the order of the remotes, each remote's branches grouped by name prefix
// when groupByPrefix is set (see GroupByRefNamePrefix).
func UnfoldRemoteBranches(remotes []*models.Remote, groupByPrefix bool) []*models.RemoteBranch {
	items := []*models.RemoteBranch{}
	for _, remote := range remotes {
		branches := remote.Branches
		if groupByPrefix {
			branches = GroupByRefNamePrefix(branches, remoteBranchName)
		}
		items = append(items, branches...)
	}
	return items
}

func remoteBranchName(b *models.RemoteBranch) string { return b.Name }

// RemoteListHeader is a non-selectable row of a remote branches list: the
// header of a remote (Remote set) or of a folder of branches (Prefix set),
// inserted at Index.
type RemoteListHeader struct {
	Index  int
	Remote string
	Prefix string
}

// RemoteListHeaders derives the headers of a remote branches list from the
// rows it shows (which may be a filtered subset of UnfoldRemoteBranches, so
// this only looks at the rows given): a header wherever the remote changes,
// when withRemoteHeaders is set, and a folder header above each run of
// branches sharing a name prefix, when groupByPrefix is set. It also returns
// the folder of each branch under a folder header, by full name.
func RemoteListHeaders(items []*models.RemoteBranch, withRemoteHeaders bool, groupByPrefix bool) ([]RemoteListHeader, map[string]string) {
	headers := []RemoteListHeader{}
	prefixes := map[string]string{}
	for start := 0; start < len(items); {
		end := start + 1
		for end < len(items) && items[end].RemoteName == items[start].RemoteName {
			end++
		}
		if withRemoteHeaders {
			headers = append(headers, RemoteListHeader{Index: start, Remote: items[start].RemoteName})
		}
		if groupByPrefix {
			run := items[start:end]
			for _, group := range RefGroups(run, remoteBranchName) {
				headers = append(headers, RemoteListHeader{Index: start + group.Index, Prefix: group.Prefix})
				for _, branch := range run[group.Index : group.Index+group.Size] {
					prefixes[branch.FullName()] = group.Prefix
				}
			}
		}
		start = end
	}
	return headers, prefixes
}
