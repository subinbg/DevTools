package presentation

import (
	"strings"

	"github.com/jesseduffield/lazygit/pkg/gui/presentation/icons"
	"github.com/jesseduffield/lazygit/pkg/theme"
)

// RefGroup is a run of consecutive list items whose names share a
// directory-like prefix ("feature/" for "feature/foo"). The list shows a folder
// header above the run and the items' names without the prefix, indented.
type RefGroup struct {
	Prefix string
	// Index of the first item of the group in the list
	Index int
	Size  int
}

// RefGroupIndent is prepended to the name of an item shown under a group header.
const RefGroupIndent = "  "

// RefNamePrefix returns the part of a ref name up to and including its last
// slash, or "" for names without a slash.
func RefNamePrefix(name string) string {
	i := strings.LastIndex(name, "/")
	if i < 0 {
		return ""
	}
	return name[:i+1]
}

// GroupByRefNamePrefix reorders items so that items whose names share a prefix
// are contiguous. Groups keep the position of their first item, and the order
// within a group and between everything else is preserved. Only prefixes shared
// by at least two items form a group; other items stay where they are.
func GroupByRefNamePrefix[T any](items []T, name func(T) string) []T {
	members := map[string][]T{}
	for _, item := range items {
		if prefix := RefNamePrefix(name(item)); prefix != "" {
			members[prefix] = append(members[prefix], item)
		}
	}

	result := make([]T, 0, len(items))
	placed := map[string]bool{}
	for _, item := range items {
		prefix := RefNamePrefix(name(item))
		if prefix == "" || len(members[prefix]) < 2 {
			result = append(result, item)
			continue
		}
		if !placed[prefix] {
			placed[prefix] = true
			result = append(result, members[prefix]...)
		}
	}
	return result
}

// RefGroups finds the groups (runs of at least two consecutive items sharing a
// name prefix) in a list that GroupByRefNamePrefix has ordered. It works on the
// displayed list, so filtering the list only shrinks or drops groups.
func RefGroups[T any](items []T, name func(T) string) []RefGroup {
	groups := []RefGroup{}
	for i := 0; i < len(items); {
		prefix := RefNamePrefix(name(items[i]))
		j := i + 1
		for prefix != "" && j < len(items) && RefNamePrefix(name(items[j])) == prefix {
			j++
		}
		if prefix != "" && j-i >= 2 {
			groups = append(groups, RefGroup{Prefix: prefix, Index: i, Size: j - i})
		}
		i = j
	}
	return groups
}

// RefGroupPrefixes maps the name of every item that sits under a group header
// to that group's prefix, so the item can be shown without it.
func RefGroupPrefixes[T any](items []T, name func(T) string) map[string]string {
	result := map[string]string{}
	for _, group := range RefGroups(items, name) {
		for _, item := range items[group.Index : group.Index+group.Size] {
			result[name(item)] = group.Prefix
		}
	}
	return result
}

// FormatRefGroupHeader renders the folder header shown above a group.
func FormatRefGroupHeader(prefix string) string {
	marker := "▾"
	if icons.IsIconEnabled() {
		marker = icons.DEFAULT_DIRECTORY_ICON.Icon
	}
	return theme.DefaultTextColor.SetBold().Sprint(marker + " " + prefix)
}

// refGroupDisplayName returns how an item's name is shown in the list: without
// its group prefix and indented when it sits under a group header.
func refGroupDisplayName(name string, groupPrefix string) string {
	if groupPrefix == "" {
		return name
	}
	return RefGroupIndent + strings.TrimPrefix(name, groupPrefix)
}
