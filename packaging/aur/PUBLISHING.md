# Publishing damon to the AUR

These steps run on an Arch Linux machine (macOS cannot run `makepkg`/
`namcap`). Artifacts in this directory (`PKGBUILD`, `.SRCINFO`) are the
source of truth — copy them into the AUR git checkout.

## Prerequisites
- An AUR account with your SSH public key registered
  (https://aur.archlinux.org/ -> My Account -> SSH Public Key).
- `base-devel` installed (`pacman -S --needed base-devel`).

## Build + lint locally
    mkdir -p /tmp/damon-aur && cp packaging/aur/PKGBUILD packaging/aur/.SRCINFO /tmp/damon-aur/
    cd /tmp/damon-aur
    makepkg -si            # builds from the release tarball and installs
    damon --version        # expect: damon 0.1.0
    namcap PKGBUILD
    namcap damon-0.1.0-1-*.pkg.tar.zst
    # If makepkg regenerated fields, refresh .SRCINFO:
    makepkg --printsrcinfo > .SRCINFO

## First publish (new package)
    git clone ssh://aur@aur.archlinux.org/damon.git aur-damon
    cd aur-damon
    cp /tmp/damon-aur/PKGBUILD /tmp/damon-aur/.SRCINFO .
    git add PKGBUILD .SRCINFO
    git commit -m "damon 0.1.0"
    git push

## Updating (later releases)
Bump `pkgver`, refresh `sha256sums` (`updpkgsums`), regenerate `.SRCINFO`,
commit, push. Mirror the changes back into this repo's `packaging/aur/`.
