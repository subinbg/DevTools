# DevTools

Terminal tools I built and patched for myself.
Each tool is under `tools/<tool>/src` as a squashed git subtree and is edited in place.

| Tool | Codebase |
|------|----------|
| lazygit | https://github.com/jesseduffield/lazygit |
| zellij | https://github.com/zellij-org/zellij |

A release is a `release-<tool>-<ref>[-suffix]` tag.
Pushing one builds `<tool>-linux-x64` and `<tool>-darwin-arm64`, checks and runs each on its own platform, and attaches them with a `SHA256SUMS` file to a GitHub Release.

## Example

For example, assume that I want my own build of `bat` (https://github.com/sharkdp/bat).
Everything about a tool will be placed inside `tools/<tool>/`.
The name `<tool>` should not contain a dash.

Files to write first:

```
tools/bat/tool.env        UPSTREAM_REPO=https://github.com/sharkdp/bat.git
                          UPSTREAM_REF=v0.25.0
tools/bat/Dockerfile      stages: base (the toolchain), test, build (ARG TARGET, ARG COMMIT), export
tools/bat/.dockerignore   dist and src/target
```

Then:

```sh
git add tools/bat && git commit -m "Add the bat tool"   # add needs a clean tree
./devtools add bat                 # import upstream v0.25.0 into tools/bat/src, two commits
./devtools test bat                # the test stage
./devtools build bat darwin-arm64  # -> tools/bat/dist/bat-darwin-arm64 (linux-x64 is the default)
./devtools shell bat               # a shell in the toolchain image
```

Patch `tools/bat/src/` and commit as in any repository. Later:

```sh
./devtools diff bat                # my changes against pristine upstream
./devtools status                  # pinned refs vs the latest upstream tags
./devtools update bat v0.26.0      # merge the newer upstream and pin it
./devtools release bat             # tag release-bat-v0.26.0 and push it; CI publishes the Release
./devtools clean bat               # drop dist/, src/target, the shell image and the build caches
```

If `update` stops on conflicts, resolve them under `tools/bat/src`, commit, and run `./devtools pin bat v0.26.0`.
