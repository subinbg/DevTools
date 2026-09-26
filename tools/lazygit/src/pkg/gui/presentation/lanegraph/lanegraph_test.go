package lanegraph

import (
	"strings"
	"testing"

	"github.com/jesseduffield/lazygit/pkg/commands/models"
	"github.com/jesseduffield/lazygit/pkg/utils"
	"github.com/samber/lo"
	"github.com/stretchr/testify/assert"
)

func makeCommits(opts []models.NewCommitOpts) []*models.Commit {
	pool := &utils.StringPool{}
	return lo.Map(opts, func(o models.NewCommitOpts, _ int) *models.Commit {
		return models.NewCommit(pool, o)
	})
}

func renderPlain(g *Graph, pad bool) string {
	width := 0
	if pad {
		width = g.Width()
	}
	lines := make([]string, 0, g.NumRows())
	for i := range g.NumRows() {
		lines = append(lines, strings.TrimRight(g.RenderRowPlain(i, width), " "))
	}
	return strings.Join(lines, "\n")
}

func expected(s string) string {
	lines := strings.Split(strings.Trim(s, "\n"), "\n")
	return strings.Join(lo.Map(lines, func(line string, _ int) string {
		return strings.TrimRight(strings.TrimPrefix(line, "\t\t\t"), " ")
	}), "\n")
}

func TestBuild(t *testing.T) {
	scenarios := []struct {
		name     string
		commits  []models.NewCommitOpts
		opts     Options
		expected string
		lanes    map[string]int
	}{
		{
			name: "linear history",
			commits: []models.NewCommitOpts{
				{Hash: "1", Parents: []string{"2"}},
				{Hash: "2", Parents: []string{"3"}},
				{Hash: "3"},
			},
			expected: `
			●
			●
			●`,
			lanes: map[string]int{"1": 0, "2": 0, "3": 0},
		},
		{
			name: "merged branch, date order: the merged line gets its own column and color",
			commits: []models.NewCommitOpts{
				{Hash: "1", Parents: []string{"2", "4"}},
				{Hash: "4", Parents: []string{"3"}},
				{Hash: "2", Parents: []string{"3"}},
				{Hash: "3", Parents: []string{"5"}},
				{Hash: "5"},
			},
			expected: `
			•─╮
			│ ●
			● │
			●─╯
			●`,
			lanes: map[string]int{"1": 0, "4": 1, "2": 0, "3": 0, "5": 0},
		},
		{
			name: "merged branch, topo order: the merged line runs alongside until its commits",
			commits: []models.NewCommitOpts{
				{Hash: "1", Parents: []string{"2", "4"}},
				{Hash: "2", Parents: []string{"3"}},
				{Hash: "3", Parents: []string{"5"}},
				{Hash: "4", Parents: []string{"5"}},
				{Hash: "5"},
			},
			expected: `
			•─╮
			● │
			● │
			│ ●
			●─╯`,
		},
		{
			name: "a merge whose second parent is already heading down an existing line joins it",
			commits: []models.NewCommitOpts{
				{Hash: "1", Parents: []string{"2"}},
				{Hash: "2", Parents: []string{"3", "5"}},
				{Hash: "3", Parents: []string{"4", "5"}},
				{Hash: "4", Parents: []string{"6"}},
				{Hash: "5", Parents: []string{"6"}},
				{Hash: "6"},
			},
			expected: `
			●
			•─╮
			•─┤
			● │
			│ ●
			●─╯`,
		},
		{
			name: "a merge line joining an existing line to its left",
			commits: []models.NewCommitOpts{
				{Hash: "1", Parents: []string{"2"}},
				{Hash: "3", Parents: []string{"4", "2"}},
				{Hash: "2", Parents: []string{"5"}},
				{Hash: "4", Parents: []string{"5"}},
				{Hash: "5"},
			},
			expected: `
			●
			├─•
			● │
			│ ●
			●─╯`,
		},
		{
			name: "several branch tips take the leftmost free columns, freed columns are reused",
			commits: []models.NewCommitOpts{
				{Hash: "a", Parents: []string{"m2"}},
				{Hash: "m1", Parents: []string{"m2", "f"}},
				{Hash: "b", Parents: []string{"m2"}},
				{Hash: "m2", Parents: []string{"m3"}},
				{Hash: "f", Parents: []string{"m3"}},
				{Hash: "m3"},
			},
			expected: `
			●
			│ •─╮
			│ │ │ ●
			●─┴─┼─╯
			│   ●
			●───╯`,
			lanes: map[string]int{"a": 0, "m1": 1, "b": 3, "m2": 0, "f": 2, "m3": 0},
		},
		{
			name: "a tip listed after a merge takes the next free column",
			commits: []models.NewCommitOpts{
				{Hash: "1", Parents: []string{"2", "4"}},
				{Hash: "3", Parents: []string{"5"}},
				{Hash: "4", Parents: []string{"5"}},
				{Hash: "2", Parents: []string{"5"}},
				{Hash: "5"},
			},
			expected: `
			•─╮
			│ │ ●
			│ ● │
			● │ │
			●─┴─╯`,
		},
		{
			name: "a merge line crossing a passing line",
			commits: []models.NewCommitOpts{
				{Hash: "a", Parents: []string{"d"}},
				{Hash: "b", Parents: []string{"e"}},
				{Hash: "c", Parents: []string{"f", "d"}},
				{Hash: "d"},
				{Hash: "e"},
				{Hash: "f"},
			},
			expected: `
			●
			│ ●
			├─┼─•
			● │ │
			  ● │
			    ●`,
		},
		{
			name: "octopus merge",
			commits: []models.NewCommitOpts{
				{Hash: "1", Parents: []string{"2", "3", "4"}},
				{Hash: "2", Parents: []string{"5"}},
				{Hash: "3", Parents: []string{"5"}},
				{Hash: "4", Parents: []string{"5"}},
				{Hash: "5"},
			},
			expected: `
			•─┬─╮
			● │ │
			│ ● │
			│ │ ●
			●─┴─╯`,
		},
		{
			name: "working-tree row above HEAD, with a newer commit on another branch listed first",
			commits: []models.NewCommitOpts{
				{Hash: "other", Parents: []string{"base"}},
				{Hash: "head", Parents: []string{"base"}},
				{Hash: "base"},
			},
			opts: Options{WorkingTree: true, HeadHash: "head"},
			expected: `
			◌
			┆ ●
			● │
			●─╯`,
			lanes: map[string]int{"other": 1, "head": 0, "base": 0},
		},
		{
			name: "working-tree row without a known HEAD is a lone node",
			commits: []models.NewCommitOpts{
				{Hash: "1", Parents: []string{"2"}},
				{Hash: "2"},
			},
			opts: Options{WorkingTree: true},
			expected: `
			◌
			●
			●`,
		},
		{
			name: "a parent beyond the loaded commits keeps its line open to the bottom",
			commits: []models.NewCommitOpts{
				{Hash: "1", Parents: []string{"2", "x"}},
				{Hash: "2", Parents: []string{"3"}},
				{Hash: "3", Parents: []string{"y"}},
			},
			expected: `
			•─╮
			● │
			● │`,
		},
		{
			name: "duplicate parents are ignored",
			commits: []models.NewCommitOpts{
				{Hash: "1", Parents: []string{"2", "2"}},
				{Hash: "2"},
			},
			expected: `
			•
			●`,
		},
	}

	for _, s := range scenarios {
		t.Run(s.name, func(t *testing.T) {
			g := Build(makeCommits(s.commits), s.opts)
			assert.Equal(t, expected(s.expected), renderPlain(g, false))
			for hash, lane := range s.lanes {
				actual, ok := g.LaneOf(hash)
				assert.True(t, ok, hash)
				assert.Equal(t, lane, actual, "lane of %s", hash)
			}
		})
	}
}

func TestRenderRowPadding(t *testing.T) {
	g := Build(makeCommits([]models.NewCommitOpts{
		{Hash: "1", Parents: []string{"2", "3"}},
		{Hash: "3", Parents: []string{"2"}},
		{Hash: "2", Parents: []string{"4"}},
		{Hash: "4"},
	}), Options{})

	assert.Equal(t, 2, g.NumLanes())
	assert.Equal(t, 4, g.Width())
	assert.Equal(t, "•─╮ ", g.RenderRowPlain(0, g.Width()))
	assert.Equal(t, "│ ● ", g.RenderRowPlain(1, g.Width()))
	assert.Equal(t, "●─╯ ", g.RenderRowPlain(2, g.Width()))
	assert.Equal(t, "●   ", g.RenderRowPlain(3, g.Width()))
	assert.Equal(t, "● ", g.RenderRowPlain(3, 0))
	assert.Equal(t, "", g.RenderRowPlain(7, 0), "out of range rows render empty")
}

func TestEmptyGraph(t *testing.T) {
	g := Build(nil, Options{WorkingTree: true})
	assert.Equal(t, 0, g.NumRows())
	assert.False(t, g.HasWorkingTreeRow())
	assert.Nil(t, g.LaneStyle("1"))
}

func TestLaneStyles(t *testing.T) {
	g := Build(makeCommits([]models.NewCommitOpts{
		{Hash: "1", Parents: []string{"2", "3"}},
		{Hash: "3", Parents: []string{"2"}},
		{Hash: "2"},
	}), Options{WorkingTree: true, HeadHash: "1"})

	styles := g.LaneStyles()
	assert.Len(t, styles, 3, "the working-tree row has no lane style")
	assert.Same(t, styles["1"], g.LaneStyle("1"))
	assert.NotSame(t, styles["1"], styles["3"], "lanes have different colors")
	assert.Same(t, styles["1"], styles["2"])
}
