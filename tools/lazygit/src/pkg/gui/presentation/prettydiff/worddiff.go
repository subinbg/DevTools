package prettydiff

import (
	"unicode"
	"unicode/utf8"
)

// Span is a byte range [Start, End) of a line's text to highlight, because the
// words in it were changed.
type Span struct {
	Start, End int
}

// Lines with more tokens than this are compared no further: the quadratic
// comparison would cost more than the highlight is worth.
const maxTokens = 400

// If highlighting would cover more than this share of a pair of lines, the
// lines are too different for the highlight to mean anything, so they get
// none.
const maxChangedRatio = 0.6

// Blocks with more line pairings to consider than this are paired by
// position rather than by similarity.
const maxSimilarityPairings = 256

type token struct {
	start, end int
}

// tokenize splits a line into words (runs of letters, digits and
// underscores), runs of whitespace, and single other characters.
func tokenize(s string) []token {
	tokens := make([]token, 0, len(s)/3)
	kind := func(r rune) int {
		switch {
		case r == '_' || unicode.IsLetter(r) || unicode.IsDigit(r):
			return 1
		case unicode.IsSpace(r):
			return 2
		default:
			return 0
		}
	}
	i := 0
	for i < len(s) {
		r, size := utf8.DecodeRuneInString(s[i:])
		k := kind(r)
		j := i + size
		if k != 0 {
			for j < len(s) {
				r2, size2 := utf8.DecodeRuneInString(s[j:])
				if kind(r2) != k {
					break
				}
				j += size2
			}
		}
		tokens = append(tokens, token{start: i, end: j})
		i = j
	}
	return tokens
}

// commonTokens marks the tokens of a and b that are part of a longest common
// subsequence of the two token lists.
func commonTokens(a string, ta []token, b string, tb []token) ([]bool, []bool) {
	n, m := len(ta), len(tb)
	// lengths[i][j] is the LCS length of ta[i:] and tb[j:]
	lengths := make([]int32, (n+1)*(m+1))
	idx := func(i, j int) int { return i*(m+1) + j }
	for i := n - 1; i >= 0; i-- {
		for j := m - 1; j >= 0; j-- {
			if a[ta[i].start:ta[i].end] == b[tb[j].start:tb[j].end] {
				lengths[idx(i, j)] = lengths[idx(i+1, j+1)] + 1
			} else {
				lengths[idx(i, j)] = max(lengths[idx(i+1, j)], lengths[idx(i, j+1)])
			}
		}
	}

	inA := make([]bool, n)
	inB := make([]bool, m)
	i, j := 0, 0
	for i < n && j < m {
		if a[ta[i].start:ta[i].end] == b[tb[j].start:tb[j].end] {
			inA[i] = true
			inB[j] = true
			i++
			j++
		} else if lengths[idx(i+1, j)] >= lengths[idx(i, j+1)] {
			i++
		} else {
			j++
		}
	}
	return inA, inB
}

func spansOfUncommon(tokens []token, common []bool) []Span {
	var spans []Span
	for i, t := range tokens {
		if common[i] {
			continue
		}
		if len(spans) > 0 && spans[len(spans)-1].End == t.start {
			spans[len(spans)-1].End = t.end
		} else {
			spans = append(spans, Span{Start: t.start, End: t.end})
		}
	}
	return spans
}

func spannedBytes(spans []Span) int {
	total := 0
	for _, s := range spans {
		total += s.End - s.Start
	}
	return total
}

// WordSpans compares a removed line with the added line that replaced it and
// returns the ranges of each that changed. ok is false when the lines are too
// long or too different to highlight usefully.
func WordSpans(oldLine, newLine string) (oldSpans []Span, newSpans []Span, ok bool) {
	if oldLine == newLine {
		return nil, nil, true
	}
	ta, tb := tokenize(oldLine), tokenize(newLine)
	if len(ta) > maxTokens || len(tb) > maxTokens || len(ta) == 0 || len(tb) == 0 {
		return nil, nil, false
	}
	inA, inB := commonTokens(oldLine, ta, newLine, tb)
	oldSpans = spansOfUncommon(ta, inA)
	newSpans = spansOfUncommon(tb, inB)

	changed := float64(spannedBytes(oldSpans) + spannedBytes(newSpans))
	total := float64(len(oldLine) + len(newLine))
	if total > 0 && changed/total > maxChangedRatio {
		return nil, nil, false
	}
	return oldSpans, newSpans, true
}

// similarity is the share of a pair of lines that is common to both.
func similarity(oldLine, newLine string) float64 {
	oldSpans, newSpans, ok := WordSpans(oldLine, newLine)
	if !ok {
		return 0
	}
	total := float64(len(oldLine) + len(newLine))
	if total == 0 {
		return 1
	}
	return 1 - float64(spannedBytes(oldSpans)+spannedBytes(newSpans))/total
}

// BlockSpans highlights a block of removed lines that is followed by a block
// of added lines. Lines are paired by position when there are as many of each,
// and otherwise by similarity (keeping their order); lines left without a
// partner are not highlighted. It returns the spans of each removed line and
// of each added line.
func BlockSpans(dels []string, adds []string) ([][]Span, [][]Span) {
	delSpans := make([][]Span, len(dels))
	addSpans := make([][]Span, len(adds))
	if len(dels) == 0 || len(adds) == 0 {
		return delSpans, addSpans
	}

	pair := func(i, j int) {
		if o, n, ok := WordSpans(dels[i], adds[j]); ok {
			delSpans[i], addSpans[j] = o, n
		}
	}

	if len(dels) == len(adds) || len(dels)*len(adds) > maxSimilarityPairings {
		for i := 0; i < min(len(dels), len(adds)); i++ {
			pair(i, i)
		}
		return delSpans, addSpans
	}

	// Pair by similarity, in order: for each removed line pick the most
	// similar added line after the one paired last.
	j := 0
	for i := range dels {
		if j >= len(adds) {
			break
		}
		// leave enough added lines for the remaining removed lines when there
		// are more added than removed lines; and vice versa
		bestJ, bestScore := -1, 0.0
		lastJ := len(adds) - 1
		if len(dels) > len(adds) {
			lastJ = j
		} else {
			lastJ = len(adds) - (len(dels) - i)
		}
		for k := j; k <= lastJ && k < len(adds); k++ {
			if score := similarity(dels[i], adds[k]); score > bestScore {
				bestJ, bestScore = k, score
			}
		}
		if bestJ == -1 {
			if len(dels) > len(adds) {
				// this removed line has no partner; keep the added line for
				// a later one
				continue
			}
			j++
			continue
		}
		pair(i, bestJ)
		j = bestJ + 1
	}
	return delSpans, addSpans
}
