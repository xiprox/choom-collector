#!/usr/bin/env bash
# Read-only probe of the WSL distro: OS, sudo, Rust, and the dev packages gpui needs.
. /etc/os-release; echo "os: $PRETTY_NAME"
if sudo -n true 2>/dev/null; then echo "sudo: passwordless"; else echo "sudo: needs password"; fi
echo "cargo: $(command -v cargo || ls ~/.cargo/bin/cargo 2>/dev/null || echo none)"
~/.cargo/bin/rustc --version 2>/dev/null || rustc --version 2>/dev/null || echo "rustc: none"
echo "cc: $(command -v cc || echo none)  pkg-config: $(command -v pkg-config || echo none)  cmake: $(command -v cmake || echo none)"
for p in build-essential pkg-config libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-dev libx11-xcb-dev libxcb1-dev libfontconfig-dev libfreetype-dev libvulkan1 libvulkan-dev libssl-dev clang; do
  s=$(dpkg-query -W -f='${Status}' "$p" 2>/dev/null | grep -q "install ok installed" && echo yes || echo no)
  echo "pkg $p: $s"
done
nproc; free -g | head -2; df -h ~ | tail -1
