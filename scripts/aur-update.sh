#!/usr/bin/env bash
# After a release: set pkgver and the checksums in both PKGBUILDs and regenerate .SRCINFO.
# Usage: scripts/aur-update.sh 0.1.0   (needs curl, makepkg on an Arch system)
set -euo pipefail
cd "$(dirname "$0")/../packaging/aur"
V=${1:?version, e.g. 0.1.0}
R=https://github.com/nishiki001/cloudberry
sums=$(curl -fsSL "$R/releases/download/v$V/SHA256SUMS")
sum() { echo "$sums" | awk -v f="$1" '$2==f {print $1}'; }
src=$(curl -fsSL "$R/archive/v$V.tar.gz" | sha256sum | cut -d' ' -f1)
sed -i "s/^pkgver=.*/pkgver=$V/; s/^pkgrel=.*/pkgrel=1/" cloudberry-bin/PKGBUILD cloudberry/PKGBUILD
sed -i "s/^sha256sums_x86_64=.*/sha256sums_x86_64=('$(sum cloudberry-linux-x86_64.tar.gz)')/; s/^sha256sums_aarch64=.*/sha256sums_aarch64=('$(sum cloudberry-linux-aarch64.tar.gz)')/" cloudberry-bin/PKGBUILD
sed -i "s/^sha256sums=.*/sha256sums=('$src')/" cloudberry/PKGBUILD
for d in cloudberry-bin cloudberry; do (cd $d && makepkg --printsrcinfo > .SRCINFO); done
echo "now: cd packaging/aur/<pkg> and push to ssh://aur@aur.archlinux.org/<pkg>.git"
