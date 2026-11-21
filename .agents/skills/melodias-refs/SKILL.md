---
name: melodias-refs
description: Build and use the melodias-studio reference library, covering the concept folders, how the pipeline picks references, the fetch, import and generate commands, Pixabay and OpenGameArt rules, the manifest and deduplication, and the library root fallback. Use this whenever thumbnails ignore the references, a concept is not matched by the vibe, you need CC0 art or generated references, a fetch fails for a missing key or license, or you touch src/refs/, src/references.rs or IMAGE_REFERENCES.
---

# Melodias reference library

The reference library is an optional folder of images grouped into concept subfolders. It only
conditions the thumbnail; it is not copied into the repository, and the app never mounts a
share or stores credentials. The library is user supplied and usually lives outside the repo,
on a normal folder or a mounted SMB share.

```
<library>/
  battle/       pixabay-1a2b3c4d.png  import-...png  generated-...png
  castle/
  forest/
  ...
  sources.jsonl
```

`sources.jsonl` sits at the library root, one JSON object per stored file, with concept, file,
source, license, author, original URL and `sha256`. Empty optional fields are omitted.

## How the pipeline uses it

`src/references.rs`:

- The concept directory is `IMAGE_REFERENCE_CONCEPT` when set and the folder exists. Otherwise
  the folder whose normalized name is contained in the vibe wins, and the longest match beats a
  shorter one. Otherwise the library root itself is used.
- Only `png`, `jpg`, `jpeg` and `webp` files count, sorted by name.
- `IMAGE_REFERENCE_COUNT` (default 3) files are sent per generation.
- Retry rounds rotate the subset with an offset, so a second round sees different references.
- An unreadable or missing folder disables references with a warning; it does not abort the run.

The concept folder names must be single words the vibes actually use. The fixed vocabulary is
`CONCEPTS` in `src/refs/mod.rs`: battle, forest, castle, jrpg, boss, dungeon, cave, village,
desert, snow, volcano, space, sky, underwater, swamp, graveyard, temple and city.

## Commands

```bash
melodias refs fetch [--source pixabay|opengameart] [--concept NAME]...
melodias refs import --concept NAME --license TEXT [options] SOURCE...
melodias refs generate [--concept NAME]... [--count N]
make refs ARGS="fetch --source opengameart --concept castle"
```

All three accept `--out DIR` and `--dry-run`. The shortcut is `make refs ARGS="..."`; there are
no separate make targets per subcommand.

### fetch

- `--source` defaults to `pixabay`; `opengameart` is the other adapter.
- `--concept` is repeatable. Without it, the source's default list is used.
- `--query` only works with exactly one concept in `fetch`.
- `--count` is files per concept, default 40.
- `--tag` keeps only candidates whose tags contain the text.
- `--max-files-per-pack` 60, `--max-bytes` 6000000, `--max-pack-bytes` 40000000,
  `--size` 768, `--delay` 0.2 seconds.
- `--packs` also imports the zip behind an entry. Packs are opt-in because their members are
  often body parts, icons or single props, not scenes.

### import

- `--concept` and `--license` are required.
- Each `SOURCE` is an image, a folder searched recursively, or a `.zip` pack. Packs are read
  with the `zip` crate; `unzip` is not required.
- `--author`, `--source-url`, `--prefix` (default `import`) and `--size` (default 768) are
  optional. Use the real license string; it is written to the manifest.

### generate

- Needs sd-server answering at `SD_URL`; `--dry-run` prints the prompts and seeds without
  contacting it.
- `--concept` defaults to all concepts; `--count` defaults to 3 and values above 50 are
  rejected.
- Uses `IMAGE_STYLE` and `IMAGE_NEGATIVE_PROMPT`; `IMAGE_SEED` makes a batch reproducible.
- Output is stored as `generated-<hash>.png` with license `generated`.

## Sources and keys

Pixabay reads the key from `PIXABAY_API_KEY`, then `pixabay_api_key` in `config.toml`, then
`$XDG_CONFIG_HOME/melodias-studio/pixabay.key` (falling back to `~/.config/...`). It stores the
license string `pixabay-content-license`. Without a key, `fetch --source pixabay` fails with the
three options in the message.

OpenGameArt needs no key. It searches CC0 2D art only (license id 4), re-checks the license on
each art page and uses a browser User-Agent, because the site rejects generic clients. Entries
store `CC0-1.0`. Do not relax the CC0 filter; the channel publishes original material.

## Storage and deduplication

`fetch`, `import` and `generate` all normalize through ImageMagick (`magick`, falling back to
`convert`): first frame, auto-orient, longest side at most `--size`, no upscale, stripped
metadata, PNG. A machine without ImageMagick fails these commands. The test suite itself does
not need it.

Each stored file is named `<source>-<first 8 of sha256>.<ext>` inside its concept folder. The
library keeps the hash set from `sources.jsonl`, so the same image is never stored twice, even
from two sources or two concepts. The manifest is append-only. Deleting a file by hand frees its
hash on the next run, so the same content can be fetched again, but the stale line stays in the
manifest; remove the line too if you want the manifest to describe the folder.

## Library root

Resolution order: `--out`, then `IMAGE_REFERENCES` or `image_references` through the normal
config precedence, then `$XDG_DATA_HOME/melodias-studio/references` or
`~/.local/share/melodias-studio/references`. `~/` is expanded for an explicit `--out`.

## Adding a source or a concept

A source is one module implementing `Source` in `src/refs/`: `name`, `defaults`, and
`candidates(concept, query, count)`. Register it in `source()`. Discovery only returns
candidates; download, normalization, deduplication and the manifest stay in `store`.

A concept is one `Concept` entry in `CONCEPTS`, with the match term and optional Pixabay and
OpenGameArt query overrides. Keep the folder name a single lowercase word. Both changes need
`make check`; the tests are `src/tests/refs_*.rs`.

## Common failures

| Symptom | Cause | Fix |
| --- | --- | --- |
| thumbnails look unrelated to the vibe | concept folder not matched | rename the folder to a word in the vibe or set `IMAGE_REFERENCE_CONCEPT` |
| `references: no images in ...` | empty concept folder | fill it or point `IMAGE_REFERENCES` at the right root |
| `no Pixabay key` | key missing | set `PIXABAY_API_KEY`, the config key or the key file |
| `missing ImageMagick (magick) on PATH` | normalization needs it | install ImageMagick |
| `unknown source` | typo | use `pixabay` or `opengameart` |
| `--query needs exactly one --concept` | more than one `--concept` | split the command |

## Verification

- `melodias refs fetch --dry-run` and `melodias refs generate --dry-run` write nothing.
- After a fetch: check `sources.jsonl`, the concept folder and that no duplicate hash appears.
- `cargo test refs` covers the adapters, the concept matching, the store and the CLI parsing.
- Confirm what the pipeline would send by reading the log line
  `references: N images from <library>`.
