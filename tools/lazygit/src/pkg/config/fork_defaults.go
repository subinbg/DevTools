package config

// ApplyUpstreamDefaults resets the settings whose defaults this fork changed
// back to upstream lazygit's values. The integration test harness uses it so
// that upstream's tests keep passing unchanged.
func ApplyUpstreamDefaults(cfg *UserConfig) {
	// commits view
	cfg.Gui.ShowRefLabelsInCommitsView = false
	cfg.Git.Log.ShowWholeGraph = false

	// sidebar
	cfg.Gui.GroupBranchesByPrefix = false
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
