package git_commands

import (
	"strings"

	"github.com/jesseduffield/lazygit/pkg/commands/models"
)

// parseDecorations parses the %D placeholder of `git log --decorate=full`,
// e.g. "HEAD -> refs/heads/main, refs/remotes/origin/main, tag: refs/tags/v1.0.0",
// into typed refs. It also returns the decorations in git's short form
// ("HEAD -> main, origin/main, tag: v1.0.0"), which is what we show and search
// as a commit's ExtraInfo. Short-form input is accepted too, in which case
// branches can't be classified and become CommitRefOther.
func parseDecorations(decorations string) ([]models.CommitRef, string) {
	fields := strings.Split(decorations, ",")
	refs := make([]models.CommitRef, 0, len(fields))
	short := make([]string, 0, len(fields))

	for _, field := range fields {
		field = strings.TrimSpace(field)
		if field == "" {
			continue
		}

		name, isHead := strings.CutPrefix(field, "HEAD -> ")
		headPrefix := ""
		if isHead {
			headPrefix = "HEAD -> "
		}

		switch {
		case name == "HEAD":
			refs = append(refs, models.CommitRef{Kind: models.CommitRefDetachedHead, Name: "HEAD", IsHead: true})
			short = append(short, "HEAD")
		case strings.HasPrefix(name, "tag: "):
			tag := strings.TrimPrefix(strings.TrimPrefix(name, "tag: "), "refs/tags/")
			refs = append(refs, models.CommitRef{Kind: models.CommitRefTag, Name: tag})
			short = append(short, "tag: "+tag)
		case strings.HasPrefix(name, "refs/heads/"):
			branch := strings.TrimPrefix(name, "refs/heads/")
			refs = append(refs, models.CommitRef{Kind: models.CommitRefLocalBranch, Name: branch, IsHead: isHead})
			short = append(short, headPrefix+branch)
		case strings.HasPrefix(name, "refs/remotes/"):
			remoteBranch := strings.TrimPrefix(name, "refs/remotes/")
			short = append(short, remoteBranch)
			if strings.HasSuffix(remoteBranch, "/HEAD") {
				// e.g. origin/HEAD, a symbolic ref that duplicates the remote's default branch
				continue
			}
			refs = append(refs, models.CommitRef{Kind: models.CommitRefRemoteBranch, Name: remoteBranch})
		case name == "grafted" || name == "replaced":
			// markers git adds for shallow clones and replace refs; not refs
			short = append(short, name)
		default:
			refs = append(refs, models.CommitRef{Kind: models.CommitRefOther, Name: name, IsHead: isHead})
			short = append(short, headPrefix+name)
		}
	}

	if len(refs) == 0 {
		refs = nil
	}

	return refs, strings.Join(short, ", ")
}
