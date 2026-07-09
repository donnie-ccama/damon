# Packaging

## Homebrew

Tap: `donnie-ccama/homebrew-damon`, formula `Formula/damon.rb`.

    brew install donnie-ccama/damon/damon        # latest tagged release
    brew install --HEAD donnie-ccama/damon/damon # build main from source

The repository is public, so the versioned formula uses a GitHub release
tarball `url` + `sha256`; `head` still builds `main`.

### Cutting a release

1. Tag and push: `git tag vX.Y.Z && git push origin vX.Y.Z`.
2. sha256: `curl -sL https://github.com/donnie-ccama/damon/archive/refs/tags/vX.Y.Z.tar.gz | shasum -a 256`.
3. Update `url` + `sha256` in the tap's `Formula/damon.rb`.
4. `brew audit --strict --online damon`, then `brew install donnie-ccama/damon/damon` to verify.
5. Commit + push the tap.

## AUR

Package `damon` (source build from the release tarball). Artifacts live in
`packaging/aur/` (`PKGBUILD`, `.SRCINFO`); publishing steps are in
`packaging/aur/PUBLISHING.md`. AUR publish runs on Arch.
