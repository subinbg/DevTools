# DevTools

Monorepo of development tools I build myself for Linux x64. Each tool's upstream source is
vendored with `git subtree --squash`, so it can be edited in place and later merged with a
newer upstream release. GitHub Actions builds every tool and publishes binaries.

## Layout

```
tools/<tool>/tool.env   upstream repo + pinned ref
tools/<tool>/build.sh   builds tools/<tool>/dist/<tool> for Linux x64
tools/<tool>/src/       upstream source (git subtree); edit freely
scripts/tool.sh         add / update / diff / status / release
scripts/package.sh      turns dist/<tool> into <tool>-linux-x64{,.tar.gz,.sha256}
.github/workflows/      one workflow per tool
```

| Tool | Upstream | Build |
|------|----------|-------|
| lazygit | https://github.com/jesseduffield/lazygit | Go, `CGO_ENABLED=0`, static |

## Daily workflow

```sh
scripts/tool.sh status                  # pinned ref vs latest upstream tag
scripts/tool.sh diff lazygit            # my local changes vs pristine upstream
scripts/tool.sh update lazygit v0.66.0  # merge a newer upstream release, then commit the new pin
scripts/tool.sh release lazygit         # tag lazygit-v0.65.1 and push -> GitHub Release
scripts/tool.sh release lazygit 2       # tag lazygit-v0.65.1-2 (a second build of the same upstream)
```

Edit anything under `tools/<tool>/src/` and commit as usual. On push to `main`, the affected
tool's workflow runs the unit tests, builds it and uploads a `<tool>-linux-x64` artifact.
Pushing a `<tool>-*` tag (what `release` does) additionally attaches the binary, a tarball and
a checksum to a GitHub Release:

```sh
curl -fL https://github.com/subinbg/DevTools/releases/download/lazygit-v0.65.1/lazygit-linux-x64 -o ~/.local/bin/lazygit
chmod +x ~/.local/bin/lazygit
```

## Updating a tool

`scripts/tool.sh update <tool> <ref>` runs `git subtree pull --squash`. Upstream changes are
merged with local modifications; conflicts are ordinary git merge conflicts under
`tools/<tool>/src/`. After resolving and committing, run `scripts/tool.sh pin <tool> <ref>`.

## Adding a tool

1. Create `tools/<name>/tool.env` with `UPSTREAM_REPO` and `UPSTREAM_REF`.
2. Create `tools/<name>/build.sh` that writes `tools/<name>/dist/<name>`.
3. Commit, then `scripts/tool.sh add <name>`.
4. Copy one of the workflows in `.github/workflows/` and adjust the toolchain steps.

## Building locally

```sh
tools/lazygit/build.sh                            # Linux x64 (needs Go)
GOOS=darwin GOARCH=arm64 tools/lazygit/build.sh   # native macOS build for trying it out
```
