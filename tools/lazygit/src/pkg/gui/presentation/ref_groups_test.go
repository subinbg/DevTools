package presentation

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func id(s string) string { return s }

func TestGroupByRefNamePrefix(t *testing.T) {
	scenarios := []struct {
		testName string
		names    []string
		expected []string
	}{
		{
			testName: "no prefixes",
			names:    []string{"main", "develop"},
			expected: []string{"main", "develop"},
		},
		{
			testName: "groups keep the position of their first member, members keep their order",
			names:    []string{"main", "feature/b", "fix/x", "staging", "feature/a", "fix/y", "feature/c"},
			expected: []string{"main", "feature/b", "feature/a", "feature/c", "fix/x", "fix/y", "staging"},
		},
		{
			testName: "a prefix with a single member is not grouped",
			names:    []string{"feature/a", "main", "chore/x", "feature/b"},
			expected: []string{"feature/a", "feature/b", "main", "chore/x"},
		},
		{
			testName: "nested prefixes group by their full directory",
			names:    []string{"feature/ui/a", "feature/b", "feature/ui/c", "feature/d"},
			expected: []string{"feature/ui/a", "feature/ui/c", "feature/b", "feature/d"},
		},
	}

	for _, s := range scenarios {
		t.Run(s.testName, func(t *testing.T) {
			assert.Equal(t, s.expected, GroupByRefNamePrefix(s.names, id))
		})
	}
}

func TestRefGroups(t *testing.T) {
	names := []string{"main", "feature/b", "feature/a", "fix/x", "fix/y", "staging", "chore/z"}
	assert.Equal(t, []RefGroup{
		{Prefix: "feature/", Index: 1, Size: 2},
		{Prefix: "fix/", Index: 3, Size: 2},
	}, RefGroups(names, id))

	assert.Equal(t, map[string]string{
		"feature/b": "feature/", "feature/a": "feature/", "fix/x": "fix/", "fix/y": "fix/",
	}, RefGroupPrefixes(names, id))

	assert.Empty(t, RefGroups([]string{}, id))
	assert.Empty(t, RefGroups([]string{"feature/only"}, id))
}

func TestRefGroupDisplayName(t *testing.T) {
	assert.Equal(t, "main", refGroupDisplayName("main", ""))
	assert.Equal(t, "  a", refGroupDisplayName("feature/a", "feature/"))
	assert.Equal(t, "  ui/a", refGroupDisplayName("feature/ui/a", "feature/"))
}
