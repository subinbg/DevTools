package config

// ApplyUpstreamDefaults resets the settings whose defaults this fork changed
// back to upstream lazygit's values. The integration test harness uses it so
// that upstream's tests keep passing unchanged.
func ApplyUpstreamDefaults(cfg *UserConfig) {
	cfg.Gui.ShowRefLabelsInCommitsView = false
	cfg.Git.Log.ShowWholeGraph = false
}
