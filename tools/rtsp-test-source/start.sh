#!/usr/bin/env bash
set -euo pipefail

if ! command -v ffmpeg >/dev/null 2>&1; then
  echo "ffmpeg is required; install it or set PATH before starting the test source" >&2
  exit 1
fi

width="${SENTINEL_TEST_WIDTH:-640}"
height="${SENTINEL_TEST_HEIGHT:-360}"
fps="${SENTINEL_TEST_FPS:-15}"
host="${SENTINEL_TEST_HOST:-127.0.0.1}"
port="${SENTINEL_TEST_PORT:-8554}"
path="${SENTINEL_TEST_PATH:-front-gate}"

echo "Publishing moving FFmpeg test pattern to rtsp://${host}:${port}/${path} (${width}x${height}@${fps})" >&2

exec ffmpeg -hide_banner -loglevel warning \
  -re -f lavfi -i "testsrc2=size=${width}x${height}:rate=${fps}" \
  -an -c:v libx264 -preset ultrafast -tune zerolatency \
  -pix_fmt yuv420p -g "${fps}" -rtsp_transport tcp -f rtsp \
  "rtsp://${host}:${port}/${path}"
