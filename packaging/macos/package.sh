#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
output_dir="${1:-$repo_root/dist}"
version="$(awk -F'"' '/^version =/{print $2; exit}' "$repo_root/Cargo.toml")"
arch="$(uname -m)"
bundle="$output_dir/sentinel-streaming-${version}-rc-candidate-macos-${arch}"
archive="$bundle.tar.gz"

rm -rf "$bundle"
mkdir -p "$bundle/bin" "$bundle/config" "$bundle/docs" "$bundle/third-party"
if [[ "${SENTINEL_PACKAGE_SKIP_BUILD:-0}" != "1" ]]; then
  (cd "$repo_root" && cargo build --release --locked)
fi
cp "$repo_root/target/release/sentinel-streaming" "$bundle/bin/"
cp "$repo_root/packaging/macos/default-sentinel.yaml" "$bundle/config/sentinel.yaml"
cp "$repo_root/README.md" "$repo_root/LICENSE" "$bundle/"
cp "$repo_root/docs/INSTALLATION.md" "$repo_root/docs/STANDALONE_DEPLOYMENT.md" "$repo_root/docs/WEEKEND_RC.md" "$bundle/docs/"
cp "$repo_root/packaging/macos/THIRD_PARTY_NOTICES.md" "$bundle/"

if [[ -n "${MEDIAMTX_BIN:-}" ]]; then
  test -x "$MEDIAMTX_BIN" || { echo "MEDIAMTX_BIN must point to an executable" >&2; exit 2; }
  cp "$MEDIAMTX_BIN" "$bundle/bin/mediamtx"
  chmod 755 "$bundle/bin/mediamtx"
  printf 'MediaMTX supplied by packager: %s\n' "$MEDIAMTX_BIN" > "$bundle/third-party/MEDIAMTX.txt"
else
  cat > "$bundle/third-party/MEDIAMTX.txt" <<'EOF'
MediaMTX is an optional external runtime for browser playback.
This bundle does not download or redistribute MediaMTX automatically.
Install a reviewed compatible MediaMTX binary separately and configure its
API/playback endpoints in Sentinel Streaming.
EOF
fi

commit="$(cd "$repo_root" && git rev-parse HEAD)"
if (cd "$repo_root" && git diff --quiet && git diff --cached --quiet); then
  worktree_state="clean"
else
  worktree_state="dirty"
fi
if command -v shasum >/dev/null 2>&1; then
  hash_file() { shasum -a 256 "$1" | awk '{print $1}'; }
else
  hash_file() { sha256sum "$1" | awk '{print $1}'; }
fi
binary_hash="$(hash_file "$bundle/bin/sentinel-streaming")"
cat > "$bundle/BUILD-MANIFEST.txt" <<EOF
artifact: $(basename "$archive")
version: $version
architecture: $arch
commit: $commit
worktree: $worktree_state
binary_sha256: $binary_hash
security_mode: OPEN_LOCAL_TEST
status: untagged RC candidate; physical acceptance required before tagging
EOF

tar -C "$output_dir" -czf "$archive" "$(basename "$bundle")"
archive_hash="$(hash_file "$archive")"
printf '%s  %s\n' "$archive_hash" "$(basename "$archive")" > "$archive.sha256"
echo "Created $archive"
echo "SHA-256: $archive_hash"
