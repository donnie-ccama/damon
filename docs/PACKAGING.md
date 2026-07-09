# Packaging

## Homebrew (private tap)

Tap: `donnie-ccama/homebrew-damon` (private), formula `Formula/damon.rb`.

    brew install --HEAD donnie-ccama/damon/damon

- HEAD-only by design: the damon repo is private and untagged, so the
  formula builds `main` from source with the user's git credentials.
- If the tap won't resolve (private repo), tap explicitly once:
  `brew tap donnie-ccama/damon https://github.com/donnie-ccama/homebrew-damon`
- Upgrading a HEAD install: `brew upgrade --fetch-HEAD damon`.

## Cutting the first versioned release (post-M4)

1. Tag: `git tag v0.1.0 && git push origin v0.1.0`.
2. In the formula, add
   `url "https://github.com/donnie-ccama/damon/archive/refs/tags/v0.1.0.tar.gz"`
   and its `sha256` (private repos need `HOMEBREW_GITHUB_API_TOKEN` for the
   tarball download — or stay HEAD-only until the repo goes public).
3. `brew audit --strict damon` in the tap before pushing.

## AUR

Deferred (unchanged from the parent spec).
