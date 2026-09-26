package config

// DefaultCommitDetailsFormat is the header shown above a commit's patch: the
// refs, the subject and body, author and committer with dates, and the
// parents, in the spirit of GitKraken's commit details panel.
const DefaultCommitDetailsFormat = "tformat:%C(yellow)commit %H%C(auto)%d%C(reset)%n" +
	"%C(bold)%s%C(reset)%n%+b%n" +
	"Author:    %an <%ae>  %ad%n" +
	"Committer: %cn <%ce>  %cd%n" +
	"Parents:   %p%n"

// ApplyUpstreamDefaults resets the settings whose defaults this fork changed
// back to upstream lazygit's values. The integration test harness uses it so
// that upstream's tests keep passing unchanged.
func ApplyUpstreamDefaults(cfg *UserConfig) {
	// commits view
	cfg.Gui.ShowRefLabelsInCommitsView = false
	cfg.Git.Log.ShowWholeGraph = false
	cfg.Git.Log.Order = "topo-order"
	cfg.Git.Log.GraphStyle = "classic"

	// main panel
	cfg.Git.CommitDetailsFormat = ""
	cfg.Gui.StatusPanelView = "dashboard"
	cfg.Gui.MainViewCommitGraph = false
	cfg.Gui.PrettyDiff = false
	cfg.Gui.KeepSidePanelsWhenMainFocused = false
	cfg.Gui.MouseTextSelection = false
	cfg.Git.BranchLogCmd = "git log --graph --color=always --abbrev-commit --decorate --date=relative --pretty=medium {{branchName}} --"
	cfg.Git.AllBranchesLogCmds = []string{"git log --graph --all --color=always --abbrev-commit --decorate --date=relative  --pretty=medium"}

	// files view
	cfg.Gui.ShowStagingSectionsInFilesView = false
	cfg.Gui.ShowFileTree = true
	cfg.Gui.ShowWorkingTreeInCommitsView = false

	// sidebar
	cfg.Gui.GroupBranchesByPrefix = false
	cfg.Gui.UnfoldRemotes = false
	cfg.Gui.ShowBranchCommitHash = false
	cfg.Gui.ShowDivergenceFromBaseBranch = "none"
	cfg.Gui.ExpandFocusedSidePanel = false
	cfg.Gui.ExpandedSidePanelWeight = 2
	cfg.Gui.SidePanelWidth = 0.3333
	cfg.Gui.SidePanels = []SidePanel{
		{"status"},
		{"files", "worktrees", "submodules"},
		{"branches", "remotes", "tags"},
		{"commits", "reflog"},
		{"stash"},
	}
}
