// Package lanegraph lays out and renders a commit graph the way GitKraken
// does: every line of history keeps a stable column ("lane") from its tip
// down to the commit where it meets another line, merge lines curve into the
// lane of the branch they merged, new lines take the leftmost free lane, and
// each lane has a fixed color. Rows are one terminal line high, so curves
// become box-drawing corners within the row.
//
// The layout rules, in order, for each commit (top to bottom):
//
//  1. The commit's node goes in the leftmost lane heading for it. A commit
//     no lane heads for (a branch tip) opens a new lane in the leftmost free
//     column.
//  2. Every other lane heading for the commit ends here, curving into the
//     node. Their columns are free again from this row on.
//  3. The node's lane continues down to the first parent. Each further
//     parent either joins the lane that already heads for it (a merge line
//     curving into an existing line), or opens a new lane in the leftmost
//     free column (a column freed in step 2 counts as free).
//
// Lanes never shift sideways, so a line is a straight vertical run with
// corners only where it starts, ends or is joined. Rule 1 differs from
// GitKraken, which gives the node to the lane that started heading for the
// commit first: taking the leftmost lane instead keeps the leftmost line (the
// checked-out branch, when the working-tree row is shown) straight all the
// way down, while other lines drift left over time.
package lanegraph

import (
	"strings"

	"github.com/jesseduffield/lazygit/pkg/commands/models"
	"github.com/jesseduffield/lazygit/pkg/gui/presentation/palette"
	"github.com/jesseduffield/lazygit/pkg/gui/style"
)

// Glyphs used for the nodes. Merge commits get a small dot, like GitKraken,
// so that commits with content of their own stand out.
const (
	CommitGlyph      = '●'
	MergeGlyph       = '•'
	WorkingTreeGlyph = '◌'

	// vertical line of the (dotted) working-tree lane, above the HEAD commit
	dashedVertical = '┆'
)

// Options controls how the graph is built.
type Options struct {
	// WorkingTree adds a pseudo-commit row above the commits standing for the
	// uncommitted changes: a dotted node connected to the HEAD commit, like
	// GitKraken's WIP node. It always takes column 0, so the checked-out
	// branch ends up in the leftmost lane.
	WorkingTree bool
	// HeadHash is the hash of the HEAD commit the working-tree row connects
	// to. When it is empty or not among the commits, the working-tree row is
	// a lone node.
	HeadHash string
}

// Graph is a laid-out commit graph: one row per commit, preceded by the
// working-tree row when Options.WorkingTree is set.
type Graph struct {
	rows      []row
	laneOf    map[string]int
	numLanes  int
	hasWIPRow bool
}

// cell is one terminal cell of a rendered row. A lane occupies two cells: the
// glyph cell (node or vertical line) and, to its right, the connector cell
// that horizontal segments run through.
type cell struct {
	glyph rune // 0 for blank
	color int  // lane index the color is taken from; -1 for none
}

type row struct {
	cells []cell
}

type lane struct {
	// hash of the commit this lane is heading for
	expect string
	// true for the working-tree lane above the HEAD commit, drawn dotted
	dashed bool
}

// Build lays out the graph for the given commits, which must be in the order
// they are displayed (children before parents, as git log emits them).
func Build(commits []*models.Commit, opts Options) *Graph {
	g := &Graph{
		laneOf:    make(map[string]int, len(commits)),
		hasWIPRow: opts.WorkingTree && len(commits) > 0,
	}
	if len(commits) == 0 {
		return g
	}

	var lanes []*lane

	// column of the leftmost lane heading for the commit, or -1
	leftmostHeadingFor := func(hash string) int {
		for k, ln := range lanes {
			if ln != nil && ln.expect == hash {
				return k
			}
		}
		return -1
	}

	firstFree := func() int {
		for k, ln := range lanes {
			if ln == nil {
				return k
			}
		}
		lanes = append(lanes, nil)
		return len(lanes) - 1
	}

	numRows := len(commits)
	if g.hasWIPRow {
		numRows++
	}
	g.rows = make([]row, 0, numRows)

	layoutRow := func(hash string, parents []string, glyph rune, dashed bool) {
		// 1. the node's column
		col := leftmostHeadingFor(hash)
		expected := col >= 0
		if !expected {
			col = firstFree()
			lanes[col] = &lane{}
		}
		nodeLane := lanes[col]

		upCols := make([]bool, len(lanes))
		dashedCols := make([]bool, len(lanes))
		for k, ln := range lanes {
			if ln != nil {
				upCols[k] = true
				dashedCols[k] = ln.dashed
			}
		}
		if !expected {
			upCols[col] = false
		}

		// 2. lanes joining the node
		var joins []int
		for k, ln := range lanes {
			if ln != nil && k != col && ln.expect == hash {
				joins = append(joins, k)
				lanes[k] = nil
			}
		}

		// 3. where the node's lines go next
		var starts, mergeJoins []int
		if len(parents) == 0 {
			lanes[col] = nil
		} else {
			nodeLane.expect = parents[0]
			nodeLane.dashed = dashed
			seen := map[string]bool{parents[0]: true}
			for _, parent := range parents[1:] {
				if seen[parent] {
					continue
				}
				seen[parent] = true
				if m := leftmostHeadingFor(parent); m >= 0 {
					mergeJoins = append(mergeJoins, m)
				} else {
					m := firstFree()
					lanes[m] = &lane{expect: parent}
					starts = append(starts, m)
				}
			}
		}

		downCols := make([]bool, len(lanes))
		for k, ln := range lanes {
			downCols[k] = ln != nil
		}

		g.laneOf[hash] = col
		g.rows = append(g.rows, buildRow(col, glyph, upCols, downCols, dashedCols, joins, starts, mergeJoins))
		if len(lanes) > g.numLanes {
			g.numLanes = len(lanes)
		}
	}

	if g.hasWIPRow {
		var parents []string
		if opts.HeadHash != "" {
			for _, commit := range commits {
				if commit.Hash() == opts.HeadHash {
					parents = []string{opts.HeadHash}
					break
				}
			}
		}
		layoutRow(workingTreeHash, parents, WorkingTreeGlyph, true)
	}
	for _, commit := range commits {
		glyph := CommitGlyph
		if commit.IsMerge() {
			glyph = MergeGlyph
		}
		layoutRow(commit.Hash(), commit.Parents(), glyph, false)
	}

	return g
}

// pseudo hash of the working-tree row; never collides with a real hash
const workingTreeHash = "<working tree>"

func buildRow(
	col int,
	glyph rune,
	upCols, downCols, dashedCols []bool,
	joins, starts, mergeJoins []int,
) row {
	numCols := col + 1
	for k := range upCols {
		if upCols[k] && k+1 > numCols {
			numCols = k + 1
		}
	}
	for k := range downCols {
		if downCols[k] && k+1 > numCols {
			numCols = k + 1
		}
	}
	for _, list := range [][]int{joins, starts, mergeJoins} {
		for _, k := range list {
			if k+1 > numCols {
				numCols = k + 1
			}
		}
	}

	type conn struct {
		up, down, left, right bool
		// color of the horizontal segment passing through, when there is no
		// vertical line to take the color from
		hcolor int
	}
	conns := make([]conn, numCols)
	for k := range conns {
		conns[k].hcolor = -1
		if k < len(upCols) && upCols[k] {
			conns[k].up = true
		}
		if k < len(downCols) && downCols[k] {
			conns[k].down = true
		}
	}

	cells := make([]cell, 2*numCols)
	for i := range cells {
		cells[i].color = -1
	}

	// A horizontal segment runs from the node to another column and is
	// colored like that column: the line joining or leaving the node.
	addSegment := func(other int) {
		lo, hi := min(col, other), max(col, other)
		for k := lo; k <= hi; k++ {
			if k > lo {
				conns[k].left = true
			}
			if k < hi {
				conns[k].right = true
			}
			if conns[k].hcolor == -1 {
				conns[k].hcolor = other
			}
		}
		for x := lo; x < hi; x++ {
			connector := &cells[2*x+1]
			if connector.glyph == 0 {
				connector.glyph = '─'
				connector.color = other
			}
		}
	}
	for _, k := range joins {
		addSegment(k)
	}
	for _, k := range mergeJoins {
		addSegment(k)
	}
	for _, k := range starts {
		addSegment(k)
	}

	for k := 0; k < numCols; k++ {
		c := conns[k]
		if k == col {
			cells[2*k] = cell{glyph: glyph, color: k}
			continue
		}
		g := boxGlyph(c.up, c.down, c.left, c.right)
		if g == 0 {
			continue
		}
		color := k
		if !c.up && !c.down {
			color = c.hcolor
		}
		if g == '│' && k < len(dashedCols) && dashedCols[k] {
			g = dashedVertical
		}
		cells[2*k] = cell{glyph: g, color: color}
	}

	return row{cells: cells}
}

// boxGlyph returns the box-drawing character connecting the given sides, or
// 0 when nothing connects.
func boxGlyph(up, down, left, right bool) rune {
	switch {
	case up && down && left && right:
		return '┼'
	case up && down && left:
		return '┤'
	case up && down && right:
		return '├'
	case up && down:
		return '│'
	case up && left && right:
		return '┴'
	case up && left:
		return '╯'
	case up && right:
		return '╰'
	case up:
		return '╵'
	case down && left && right:
		return '┬'
	case down && left:
		return '╮'
	case down && right:
		return '╭'
	case down:
		return '╷'
	case left && right:
		return '─'
	case left:
		return '╴'
	case right:
		return '╶'
	default:
		return 0
	}
}

// NumRows returns the number of rows, including the working-tree row if any.
func (g *Graph) NumRows() int {
	return len(g.rows)
}

// HasWorkingTreeRow reports whether row 0 is the working-tree row.
func (g *Graph) HasWorkingTreeRow() bool {
	return g.hasWIPRow
}

// NumLanes returns the number of columns the widest row uses.
func (g *Graph) NumLanes() int {
	return g.numLanes
}

// Width returns the width in cells of the widest row.
func (g *Graph) Width() int {
	return 2 * g.numLanes
}

// RowIndexOfCommit maps a commit index (into the commits Build was given) to
// its row index.
func (g *Graph) RowIndexOfCommit(commitIdx int) int {
	if g.hasWIPRow {
		return commitIdx + 1
	}
	return commitIdx
}

// LaneOf returns the column of the commit's node.
func (g *Graph) LaneOf(hash string) (int, bool) {
	col, ok := g.laneOf[hash]
	return col, ok
}

// LaneStyle returns the color of the lane the commit's node sits in, or nil
// when the commit is not in the graph.
func (g *Graph) LaneStyle(hash string) *style.TextStyle {
	col, ok := g.laneOf[hash]
	if !ok {
		return nil
	}
	return palette.ByIndex(col)
}

// LaneStyles returns the lane color of every commit in the graph, by hash.
func (g *Graph) LaneStyles() map[string]*style.TextStyle {
	result := make(map[string]*style.TextStyle, len(g.laneOf))
	for hash, col := range g.laneOf {
		if hash == workingTreeHash {
			continue
		}
		result[hash] = palette.ByIndex(col)
	}
	return result
}

// RenderRow renders a row, colored, padded with spaces to at least minWidth
// cells (pass 0 for no padding). A row always ends with a blank cell, so text
// can follow it directly.
func (g *Graph) RenderRow(rowIdx int, minWidth int) string {
	return g.render(rowIdx, minWidth, false)
}

// RenderRowPlain renders a row without colors, for tests and plain output.
func (g *Graph) RenderRowPlain(rowIdx int, minWidth int) string {
	return g.render(rowIdx, minWidth, true)
}

func (g *Graph) render(rowIdx int, minWidth int, plain bool) string {
	if rowIdx < 0 || rowIdx >= len(g.rows) {
		return strings.Repeat(" ", minWidth)
	}
	cells := g.rows[rowIdx].cells

	builder := strings.Builder{}
	builder.Grow(4 * len(cells))

	// consecutive cells of the same color are printed with one escape sequence
	runColor := -1
	run := strings.Builder{}
	flush := func() {
		if run.Len() == 0 {
			return
		}
		if plain || runColor < 0 {
			builder.WriteString(run.String())
		} else {
			builder.WriteString(palette.ByIndex(runColor).Sprint(run.String()))
		}
		run.Reset()
	}

	for _, c := range cells {
		glyph := c.glyph
		color := c.color
		if glyph == 0 {
			glyph = ' '
			color = -1
		}
		if color != runColor {
			flush()
			runColor = color
		}
		run.WriteRune(glyph)
	}
	flush()

	for width := len(cells); width < minWidth; width++ {
		builder.WriteByte(' ')
	}

	return builder.String()
}
