# syntax=docker/dockerfile:1
# Target build stages:
#   base     the Go toolchain on Alpine
#   test     unit tests; TEST_ARGS="integration [name...]" runs integration tests instead
#   build    the binary for TARGET (linux-x64 | linux-arm64 | darwin-arm64)
#   export   that binary alone, for `docker build --output`
ARG GO_VERSION=1.25

FROM golang:${GO_VERSION}-alpine AS base

RUN apk add --no-cache bash git
# GOTOOLCHAIN=local fails when src/go.mod needs a newer Go than GO_VERSION.
ENV CGO_ENABLED=0 GOTOOLCHAIN=local
WORKDIR /work

FROM base AS source
COPY tool.env ./
COPY src ./src

FROM source AS test
ARG TEST_ARGS=""
RUN --mount=type=cache,id=lazygit-go-build,target=/root/.cache/go-build \
    cd src && set -- $TEST_ARGS && case "${1:-unit}" in \
      unit) go test ./... -short ;; \
      integration) shift && go run cmd/integration_test/main.go cli "$@" ;; \
      *) echo "TEST_ARGS: unit | integration [name...]" >&2; exit 1 ;; \
    esac

FROM source AS build
ARG TARGET=linux-x64
ARG COMMIT=unknown
RUN --mount=type=cache,id=lazygit-go-build,target=/root/.cache/go-build \
    . ./tool.env && cd src && case "$TARGET" in \
      linux-x64) export GOOS=linux GOARCH=amd64 ;; \
      linux-arm64) export GOOS=linux GOARCH=arm64 ;; \
      darwin-arm64) export GOOS=darwin GOARCH=arm64 ;; \
      *) echo "unknown TARGET '$TARGET'" >&2; exit 1 ;; \
    esac \
 && ldflags="-s -w -X main.version=${UPSTREAM_REF#v}+devtools -X main.commit=$COMMIT" \
 && ldflags="$ldflags -X main.date=$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
 && ldflags="$ldflags -X main.buildSource=binaryRelease" \
 && mkdir -p /out && go build -trimpath -ldflags "$ldflags" -o "/out/lazygit-$TARGET" .

FROM scratch AS export
COPY --from=build /out/ /
