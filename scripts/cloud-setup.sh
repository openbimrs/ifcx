#!/usr/bin/env bash
# Bootstrap an Ubuntu/Debian cloud workspace (Codex, Claude, or a fresh VM).
# Run while networking is available. See docs/cloud-setup.md for the contract.
set -euo pipefail

case "${1:-}" in
  --help|-h)
    printf 'Usage: %s\nPrepares tools and dependencies; run scripts/gate.sh separately.\n' "$0"
    exit 0 ;;
  '') ;;
  *) printf 'Unknown argument: %s\n' "$1" >&2; exit 2 ;;
esac
[[ $# -le 1 ]] || { echo 'Expected no arguments or --help' >&2; exit 2; }

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
trap 'printf "cloud setup failed at line %s: %s\n" "$LINENO" "$BASH_COMMAND" >&2' ERR

as_root() {
  if [[ "$(id -u)" == 0 ]]; then
    "$@"
  elif command -v sudo >/dev/null && sudo -n true 2>/dev/null; then
    sudo -n "$@"
  else
    echo 'Cloud setup requires root or passwordless sudo to install image dependencies.' >&2
    return 1
  fi
}

command -v apt-get >/dev/null || {
  echo 'Cloud setup supports Ubuntu/Debian images with apt-get.' >&2
  exit 1
}
as_root env DEBIAN_FRONTEND=noninteractive apt-get update
as_root env DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
  ca-certificates curl git build-essential pkg-config python3 python3-dev \
  python3-pip python3-venv xz-utils

# Make installed commands visible to later shells too: exports from a setup
# subprocess do not survive `exec ./scripts/cloud-setup.sh`.
expose() {
  [[ "$1" != "/usr/local/bin/$2" ]] || return 0
  [[ -x "$1" ]] || { echo "Installed tool missing: $1" >&2; return 1; }
  as_root mkdir -p /usr/local/bin
  as_root ln -sfn "$1" "/usr/local/bin/$2"
}
export PATH="/usr/local/bin:${CARGO_HOME:-$HOME/.cargo}/bin:$HOME/.local/bin:$PATH"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

toolchain="1.88.0" # Matches the repository's CI/MSRV.
if ! command -v rustup >/dev/null; then
  curl --fail --location --silent --show-error --retry 3 \
    https://sh.rustup.rs --output "$work/rustup-init.sh"
  bash "$work/rustup-init.sh" -y --profile minimal --default-toolchain none --no-modify-path
fi
rustup toolchain install "$toolchain" --profile minimal --component rustfmt --component clippy
# Repositories without rust-toolchain.toml still need the CI pin in future shells.
if [[ ! -f rust-toolchain.toml && ! -f rust-toolchain ]]; then
  rustup override set "$toolchain"
fi
for tool in rustup cargo rustc rustfmt cargo-fmt cargo-clippy clippy-driver; do
  if [[ -x "${CARGO_HOME:-$HOME/.cargo}/bin/$tool" ]]; then
    expose "${CARGO_HOME:-$HOME/.cargo}/bin/$tool" "$tool"
  fi
done
rustup target add --toolchain "$toolchain" wasm32-unknown-unknown
cargo fetch --locked

# Keep a compatible preinstalled Node; otherwise install a checksummed LTS.
if ! command -v node >/dev/null || ! command -v npm >/dev/null || \
  ! node -e 'const [a,b]=process.versions.node.split(".").map(Number); process.exit(a>=22 && (a!==22 || b>=12) ? 0 : 1)'; then
  node_version=22.23.3
  case "$(uname -m)" in
    x86_64) node_arch=x64 ;;
    aarch64|arm64) node_arch=arm64 ;;
    *) echo 'No Node bootstrap binary for this architecture' >&2; exit 1 ;;
  esac
  archive="node-v${node_version}-linux-${node_arch}.tar.xz"
  url="https://nodejs.org/dist/v${node_version}"
  curl --fail --location --silent --show-error --retry 3 "$url/$archive" --output "$work/$archive"
  curl --fail --location --silent --show-error --retry 3 "$url/SHASUMS256.txt" --output "$work/SHASUMS256.txt"
  (cd "$work"; grep "  $archive\$" SHASUMS256.txt > node.sha256; sha256sum --check --strict node.sha256)
  node_dir="$HOME/.local/share/openbim-cloud/node-v${node_version}-linux-${node_arch}"
  mkdir -p "$node_dir"
  tar -xJf "$work/$archive" --strip-components=1 -C "$node_dir"
  for tool in node npm npx; do expose "$node_dir/bin/$tool" "$tool"; done
fi
node --version
npm --version
npm ci --no-audit --no-fund
npm --prefix crates/openbim-ifcx-wasm/tools ci --no-audit --no-fund

# These tools belong to the disposable cloud image, not the project lockfile.
# --user avoids overwriting distro packages; Debian's externally-managed
# interpreter requires the explicit override even for user-site installs.
pip_args=(--disable-pip-version-check)
python_bin="$(python3 -c 'import sys; print(sys.prefix + "/bin")')"
if python3 -c 'import sys; sys.exit(sys.prefix != sys.base_prefix)'; then
  pip_args+=(--user --break-system-packages)
  python_bin="$(python3 -m site --user-base)/bin"
fi
python3 -m pip install "${pip_args[@]}" "uv==0.11.19" "maturin==1.13.3"
expose "$(command -v uv || printf '%s' "$python_bin/uv")" uv
expose "$(command -v maturin || printf '%s' "$python_bin/maturin")" maturin

# The CLI must exactly match the dependency recorded in Cargo.lock.
wasm_version="$(python3 - <<'PY'
import re
from pathlib import Path
matches = re.findall(r'\[\[package\]\]\nname = "wasm-bindgen"\nversion = "([^"]+)"', Path('Cargo.lock').read_text())
if len(matches) != 1:
    raise SystemExit('Expected exactly one wasm-bindgen version in Cargo.lock')
print(matches[0])
PY
)"
if [[ "$(wasm-bindgen --version 2>/dev/null || true)" != "wasm-bindgen $wasm_version" ]]; then
  # CLI build dependencies can require a newer compiler than the library MSRV.
  rustup toolchain install stable --profile minimal
  cargo +stable install wasm-bindgen-cli --version "$wasm_version" --locked --force
fi
expose "${CARGO_HOME:-$HOME/.cargo}/bin/wasm-bindgen" wasm-bindgen

# The npm gate discovers Playwright's Chromium cache without an exported
# CHROME_BIN. Use the normal browser download, never Ubuntu's snap wrapper.
browser_found=false
for browser in google-chrome-stable google-chrome chromium chromium-browser chrome; do
  if command -v "$browser" >/dev/null; then browser_found=true; break; fi
done
if [[ -n "${CHROME_BIN:-}" && -x "$CHROME_BIN" ]]; then browser_found=true; fi
if [[ "$browser_found" == false ]]; then
  # Install system libraries with privilege, but cache the browser as the user
  # who will run the gate (sudo must not send it to root's HOME).
  as_root "$(command -v npx)" --yes playwright@1.63.0 install-deps chromium
  npx --yes playwright@1.63.0 install chromium
fi

printf '\nCloud setup complete. Run: %s\n' 'scripts/gate.sh'
