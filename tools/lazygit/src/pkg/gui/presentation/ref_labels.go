package presentation

import (
	"cmp"
	"slices"
	"strings"

	"github.com/jesseduffield/generics/set"
	"github.com/jesseduffield/lazygit/pkg/commands/models"
	"github.com/jesseduffield/lazygit/pkg/gui/presentation/icons"
	"github.com/jesseduffield/lazygit/pkg/gui/presentation/palette"
	"github.com/jesseduffield/lazygit/pkg/gui/style"
	"github.com/jesseduffield/lazygit/pkg/theme"
	"github.com/samber/lo"
)

// Beyond this many labels on one commit, the rest are summarised as "+N".
const maxRefLabels = 4

const (
	checkedOutMarker = "✓"
	worktreeMarker   = "⌂"
)

var refKindOrder = map[models.CommitRefKind]int{
	models.CommitRefDetachedHead: 0,
	models.CommitRefLocalBranch:  1,
	models.CommitRefRemoteBranch: 2,
	models.CommitRefTag:          3,
	models.CommitRefOther:        4,
}

// renderRefLabels renders the refs pointing at a commit as colored labels, in
// the style of GitKraken: a filled label in the commit's graph lane color for
// local branches (bold with a check mark for the checked-out branch, with a
// house mark for a branch checked out in another worktree), a darker one for
// remote branches and a gold one for tags. The result ends with a space so it
// can be prepended to the commit subject; it is empty for commits without refs.
func renderRefLabels(
	refs []models.CommitRef,
	laneStyle *style.TextStyle,
	branchesInOtherWorktrees *set.Set[string],
) string {
	if len(refs) == 0 {
		return ""
	}

	sorted := slices.Clone(refs)
	slices.SortStableFunc(sorted, func(a, b models.CommitRef) int {
		if a.IsHead != b.IsHead {
			return lo.Ternary(a.IsHead, -1, 1)
		}
		return cmp.Compare(refKindOrder[a.Kind], refKindOrder[b.Kind])
	})

	laneIdx, hasLane := palette.IndexOf(laneStyle)

	labels := make([]string, 0, min(len(sorted), maxRefLabels)+1)
	for i, ref := range sorted {
		if i == maxRefLabels {
			labels = append(labels, theme.DefaultTextColor.Sprintf("+%d", len(sorted)-maxRefLabels))
			break
		}
		idx := laneIdx
		if !hasLane {
			// graph hidden or commit outside it: fall back to a stable per-name color
			idx = palette.IndexForName(ref.Name)
		}
		labels = append(labels, renderRefLabel(ref, idx, branchesInOtherWorktrees))
	}

	return strings.Join(labels, " ") + " "
}

func renderRefLabel(ref models.CommitRef, laneIdx int, branchesInOtherWorktrees *set.Set[string]) string {
	text := ref.Name
	var labelStyle style.TextStyle

	switch ref.Kind {
	case models.CommitRefLocalBranch:
		labelStyle = palette.Label(laneIdx)
		if ref.IsHead {
			labelStyle = labelStyle.SetBold()
			text = checkedOutMarker + " " + text
		} else if branchesInOtherWorktrees != nil && branchesInOtherWorktrees.Includes(ref.Name) {
			text = lo.Ternary(icons.IsIconEnabled(), icons.IconForWorktree(false), worktreeMarker) + " " + text
		}
	case models.CommitRefRemoteBranch:
		labelStyle = palette.DimLabel(laneIdx)
	case models.CommitRefTag:
		labelStyle = palette.TagLabel
		if icons.IsIconEnabled() {
			text = icons.TAG_ICON + " " + text
		}
	case models.CommitRefDetachedHead:
		labelStyle = palette.HeadLabel.SetBold()
		text = checkedOutMarker + " " + text
	default:
		labelStyle = theme.DefaultTextColor
	}

	return labelStyle.Sprint(" " + text + " ")
}
