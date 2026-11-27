#!/usr/bin/env bash
# Refreshes site/videos.json ("More from the channel") from the live channel,
# newest upload first with its YouTube duration. The page renders the JSON and
# keeps the static list as the offline fallback. Keeps the old file when the
# fetch fails, so `make site` never breaks offline.
# Run through `make site-videos`. SITE_VIDEOS_COUNT overrides how many uploads
# are listed (6 by default).
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
out="$root/site/videos.json"
channel="https://www.youtube.com/@melodias8bit/videos"
count="${SITE_VIDEOS_COUNT:-6}"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

if ! yt-dlp --flat-playlist --dump-json --playlist-end "$count" "$channel" > "$tmp/flat.jsonl" 2>/dev/null; then
  printf 'site-videos: channel fetch failed, keeping %s\n' "${out#"$root"/}" >&2
  exit 0
fi
if [ ! -s "$tmp/flat.jsonl" ]; then
  printf 'site-videos: channel returned no videos, keeping %s\n' "${out#"$root"/}" >&2
  exit 0
fi

python3 - "$tmp/flat.jsonl" "$tmp/ids.tsv" <<'EOF'
import json, sys
with open(sys.argv[2], "w", encoding="utf-8") as out:
    for line in open(sys.argv[1], encoding="utf-8"):
        line = line.strip()
        if not line:
            continue
        video = json.loads(line)
        vid = video.get("id", "")
        title = (video.get("title") or "").replace("\t", " ").replace("\n", " ").strip()
        if vid:
            out.write(f"{vid}\t{title}\n")
EOF
if [ ! -s "$tmp/ids.tsv" ]; then
  printf 'site-videos: channel returned no videos, keeping %s\n' "${out#"$root"/}" >&2
  exit 0
fi

: > "$tmp/rows.tsv"
while IFS=$'\t' read -r id title; do
  [ -n "$id" ] || continue
  duration=$(yt-dlp --no-playlist --print "%(duration_string)s" "https://www.youtube.com/watch?v=$id" 2>/dev/null || true)
  [ -n "$duration" ] || duration="--:--"
  printf '%s\t%s\t%s\n' "$id" "$title" "$duration" >> "$tmp/rows.tsv"
done < "$tmp/ids.tsv"

python3 - "$tmp/rows.tsv" "$out" <<'EOF'
import json, sys
rows = []
for line in open(sys.argv[1], encoding="utf-8"):
    line = line.rstrip("\n")
    if not line.strip():
        continue
    vid, title, duration = line.split("\t")
    rows.append({"id": vid, "title": title, "duration": duration or "--:--"})
with open(sys.argv[2], "w", encoding="utf-8") as fh:
    json.dump(rows, fh, ensure_ascii=False, indent=2)
    fh.write("\n")
EOF

printf 'site-videos: wrote %s with %s videos\n' "${out#"$root"/}" "$(wc -l < "$tmp/rows.tsv")"
