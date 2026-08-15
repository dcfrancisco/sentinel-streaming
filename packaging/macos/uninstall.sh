#!/usr/bin/env bash
set -euo pipefail

app_root="${SENTINEL_INSTALL_ROOT:-$HOME/Library/Application Support/Sentinel Streaming}"
label="com.sentinel.streaming"
plist="$HOME/Library/LaunchAgents/$label.plist"

launchctl bootout "gui/$(id -u)/$label" 2>/dev/null || true
rm -f "$plist"
rm -rf "$app_root/runtime"
echo "Sentinel Streaming binaries and service removed."
echo "Configuration, state, artifacts, and logs were preserved under: $app_root"
echo "To remove them manually, review and delete that directory."
