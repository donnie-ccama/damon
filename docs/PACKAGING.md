# Packaging

## Homebrew

Tap: `donnie-ccama/homebrew-cortado`, formula `Formula/cortado.rb`.

    brew install donnie-ccama/cortado/cortado        # latest tagged release
    brew install --HEAD donnie-ccama/cortado/cortado # build main from source

The repository is public, so the versioned formula uses a GitHub release
tarball `url` + `sha256`; `head` still builds `main`.

### Cutting a release

1. Tag and push: `git tag vX.Y.Z && git push origin vX.Y.Z`.
2. sha256: `curl -sL https://github.com/donnie-ccama/cortado/archive/refs/tags/vX.Y.Z.tar.gz | shasum -a 256`.
3. Update `url` + `sha256` in the tap's `Formula/cortado.rb`.
4. `brew audit --strict --online cortado`, then `brew install donnie-ccama/cortado/cortado` to verify.
5. Commit + push the tap.

## AUR

Package `cortado` (source build from the release tarball). Artifacts live in
`packaging/aur/` (`PKGBUILD`, `.SRCINFO`); publishing steps are in
`packaging/aur/PUBLISHING.md`. AUR publish runs on Arch.
