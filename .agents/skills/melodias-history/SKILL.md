---
name: melodias-history
description: Read and maintain the melodias-studio SQLite history and its dashboard, covering schema v2, the outcome values, writer-side migrations, the history views, backfill limits and the read-only dashboard. Use this whenever the dashboard is missing runs, a history query shows odd outcomes, a run was not recorded, a legacy database needs migrating, you touch src/history.rs, src/backfill.rs or src/dashboard.rs, or you need to know where the database lives and who is allowed to create it.
---

# Melodias history and dashboard

The history database is a local SQLite file written by the pipeline after a run finishes. It is
never the source of truth for a run: `output/<slug>/meta.json`, `work/score.json` and
`work/judge/verdict.json` are. History is a derived, best-effort index.

Database location, in order: `MELODIAS_DB`, `db_path` in `config.toml`, then `history.db` next
to the resolved `config.toml`. See the `melodias-config` skill for config precedence. A new file
is created with mode `0600`; never commit it.

## Who writes and who only reads

| Command | Creates the file | Migrates | Writes |
| --- | --- | --- | --- |
| pipeline (stage 11) | yes | yes | yes |
| `melodias backfill` | yes | yes | yes |
| `melodias history` | no | no | no |
| `melodias dashboard` | no | no | no |

`history::open` is the writer entry point: it creates the parent directory, applies the schema,
runs `migrate`, and sets `pragma user_version`. Readers use `history::reader`, which returns
`None` when the file is absent. `melodias history` on a machine without the file prints a hint
and exits 0.

## Schema v2

`SCHEMA_VERSION = 2` in `src/history.rs`. Tables:

- `runs`: `id`, `started_at`, `slug`, `vibe`, `rubric`, `technical`, `editorial`, `total`,
  `accepted`, `outcome`, `outcome_reason`, `rounds`, `cost_usd`, `seconds`, `config_json`.
- `candidates`: `run_id`, `kind`, `idx`, `seed`, `score`, `score_json`.
- `findings`: `run_id`, optional `candidate_id`, `code`, `source`, `severity`, `metric`,
  `value`, `threshold`, `delta`, `message`.
- `patches`: `run_id`, `code`, `prompt_before`, `prompt_after`, `applied_at`.

The mode is WAL, so the database also has `-wal` and `-shm` siblings while open. Copy all three
if you move a live database.

Migration only happens on write. A version newer than `SCHEMA_VERSION` is refused with an error
instead of being downgraded. Version 2 adds `outcome` and `outcome_reason` with `alter table`,
backfills existing rows from `accepted` (`accepted` or `below_target`), then bumps
`user_version`. Readers call `has_column` so an old file still answers `runs` and `findings`
without the new columns.

## Outcomes

| Value | Written by | Meaning |
| --- | --- | --- |
| `accepted` | pipeline | the deterministic targets were met |
| `accepted_by_limit` | pipeline | the loop ended (budget, pool or no patch) and the run is still valid |
| `below_target` | backfill and the v2 migration | imported legacy run, or a run without a retry loop |

`accepted_by_limit` is not a failure; it records that the run stopped improving. The pipeline
also stores the stop reason in `outcome_reason` (`retry.stop` plus the final scores). In the
database, `accepted` is the boolean and `outcome` is the explanation; keep both when adding a
new outcome.

## Write path

The pipeline records history at the end of stage 11, after `video.mp4` and `meta.json`. It
re-scores the finished run, which costs about 20 seconds because it re-reads media with
`ffmpeg`, refreshes `work/score.json`, then calls `history::insert` in one transaction for the
run, its candidates, the findings of the final pair and the applied patches.

The write is best effort: a failure is one log line and the run stays valid. Because
`history::insert` never retries, treat a failed write as "the run happened, the index did not".
`melodias backfill` can index it later.

Findings from the pipeline are attached to the run with a null `candidate_id`; the
per-candidate findings live in each candidate's `score_json`.

## Reading

```bash
melodias history                 # last 20 runs, table
melodias history findings        # worst findings across runs
melodias history trend --by day  # day, week or month
melodias history --json          # also --csv, combinable with a view
make history ARGS=findings
```

`melodias history` caps `runs` at 20. The dashboard shows a wider window.

## Backfill

```bash
melodias backfill                # scans OUT_DIR
melodias backfill output/<dir>   # scans one explicit directory
melodias backfill --replace      # deletes the existing row for that slug first
```

Backfill scans exactly one level (`OUT_DIR/<slug>`), requires `meta.json`, `thumbnail.png` and
`audio.mp3`, and skips anything else with a message. It inserts with `seconds = 0`,
`rounds = 0`, no patches and `outcome = below_target`, because it cannot know the original retry
loop. `--replace` deletes the old run, candidates, findings and patches by slug before inserting.

A nested run such as `output/retry-check/<slug>/` is invisible to a plain `melodias backfill`.
Point the command at the parent directory instead: `melodias backfill output/retry-check`. Do
not "fix" this with a recursive scan unless you also define how duplicate slugs are resolved.

## Dashboard

```bash
melodias dashboard [--port 7373] [--limit 200]
make dashboard
```

It binds `127.0.0.1` only, opens the database read-only, and never creates or migrates it. The
read-only open has a read-write fallback for the WAL edge case where the shared memory file
cannot be mapped; the fallback still runs `SELECT`s only.

Data sources:

- `history.db` for runs, candidates, findings and patches.
- Every run directory under `OUT_DIR` for `meta.json`, `work/score.json`,
  `work/judge/verdict.json` and the media. The directory is resolved by slug, so a nested run
  works in the detail view even when backfill skipped it.

Endpoints:

| Path | Purpose |
| --- | --- |
| `/api/summary?limit=N` | totals, run window and their findings |
| `/api/run?id=N` | one run: row, candidates, findings, patches, JSON documents and artifact list |
| `/media/<run_id>/<path>` | image, audio, video, JSON or text inside that run, with byte ranges |

Notes:

- The HTML, JS and CSS are embedded with `include_str!`. Editing `dashboard/index.html`,
  `dashboard/app.js` or `dashboard/style.css` needs a rebuild; there is no Node or build step.
- A missing database renders an empty state. A database without the expected tables answers 500
  and logs the SQL error.
- Media paths are confined to the run directory and limited to an allowlist of extensions.
- `?run=<id>` opens a run directly. The page polls the file once a minute.

## Common failures

| Symptom | Cause | Fix |
| --- | --- | --- |
| dashboard shows fewer runs than `output/` | nested run or missing media | `melodias backfill output/<parent>`; check `thumbnail.png` and `audio.mp3` |
| `already in the database, use --replace` | same slug indexed | `melodias backfill --replace` or delete the row |
| `uses schema version N, newer than 2` | file written by a newer binary | use the newer binary; do not downgrade by hand |
| `outcome` is null in an old file | read before a write migrated it | any writer run fixes it |
| dashboard answers 500 for `/api/summary` | file lacks the tables | point `MELODIAS_DB` at the real file or run backfill |
| dashboard media 404 | run directory moved or `OUT_DIR` differs | start the dashboard with the same `OUT_DIR` |

## Verification

- `cargo test history`, `cargo test backfill`, `cargo test dashboard`.
- `melodias history` on a machine without a database must print a hint and exit 0.
- After a run: `jq '.retry.stop' output/<slug>/meta.json` and
  `melodias history --json | jq '.[-1]'`.
- Full gate: `make check`.
