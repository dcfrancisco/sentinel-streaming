#!/usr/bin/env bash
set -euo pipefail

bundle="$(cd "$(dirname "$0")" && pwd)"
app_root="${SENTINEL_INSTALL_ROOT:-$HOME/Library/Application Support/Sentinel Streaming}"
launch_agents="$HOME/Library/LaunchAgents"
label="com.sentinel.streaming"
config_dir="$app_root/config"
state_dir="$app_root/state"
log_dir="$HOME/Library/Logs/Sentinel Streaming"
artifact_dir="$state_dir/artifacts"
runtime_dir="$app_root/runtime"
plist="$launch_agents/$label.plist"

if [[ ! -x "$bundle/bin/sentinel-streaming" ]]; then
  echo "Package is missing bin/sentinel-streaming" >&2
  exit 2
fi

mkdir -p "$runtime_dir" "$config_dir" "$state_dir" "$artifact_dir" "$log_dir" "$launch_agents"
cp "$bundle/bin/sentinel-streaming" "$runtime_dir/sentinel-streaming"
chmod 755 "$runtime_dir/sentinel-streaming"
if [[ ! -e "$config_dir/sentinel.yaml" ]]; then
  cp "$bundle/config/sentinel.yaml" "$config_dir/sentinel.yaml"
fi

security_mode="$(awk '/^[[:space:]]*mode:[[:space:]]*/ {print $2; exit}' "$config_dir/sentinel.yaml")"
SENTINEL_BOOTSTRAP_TOKEN="${SENTINEL_BOOTSTRAP_TOKEN:-}"
if [[ "$security_mode" != "OPEN_LOCAL_TEST" && -z "${SENTINEL_BOOTSTRAP_TOKEN:-}" ]]; then
  if command -v openssl >/dev/null 2>&1; then
    SENTINEL_BOOTSTRAP_TOKEN="$(openssl rand -hex 24)"
  else
    echo "Set SENTINEL_BOOTSTRAP_TOKEN before installing." >&2
    exit 2
  fi
fi

if [[ -x "$bundle/bin/mediamtx" ]]; then
  cp "$bundle/bin/mediamtx" "$runtime_dir/mediamtx"
  chmod 755 "$runtime_dir/mediamtx"
fi

cat > "$plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>$label</string>
  <key>ProgramArguments</key><array>
    <string>$runtime_dir/sentinel-streaming</string><string>serve</string>
    <string>--config</string><string>$config_dir/sentinel.yaml</string>
  </array>
  <key>EnvironmentVariables</key><dict>
    <key>SENTINEL_BOOTSTRAP_TOKEN</key><string>$SENTINEL_BOOTSTRAP_TOKEN</string>
    <key>SENTINEL_MEDIA_ARTIFACT_ROOT</key><string>$artifact_dir</string>
  </dict>
  <key>WorkingDirectory</key><string>$app_root</string>
  <key>RunAtLoad</key><true/><key>KeepAlive</key><true/>
  <key>StandardOutPath</key><string>$log_dir/sentinel-streaming.log</string>
  <key>StandardErrorPath</key><string>$log_dir/sentinel-streaming.error.log</string>
</dict></plist>
EOF
chmod 600 "$plist"

"$runtime_dir/sentinel-streaming" check-config --config "$config_dir/sentinel.yaml"
launchctl bootout "gui/$(id -u)/$label" 2>/dev/null || true
launchctl bootstrap "gui/$(id -u)" "$plist"
launchctl kickstart -k "gui/$(id -u)/$label"

echo "Sentinel Streaming installed."
echo "Admin: http://127.0.0.1:8081/admin"
if [[ -n "${SENTINEL_BOOTSTRAP_TOKEN:-}" ]]; then
  echo "Bootstrap token: $SENTINEL_BOOTSTRAP_TOKEN"
else
  echo "Security mode: OPEN_LOCAL_TEST (local authentication disabled)"
fi
echo "Logs: $log_dir"
echo "Config: $config_dir/sentinel.yaml"
