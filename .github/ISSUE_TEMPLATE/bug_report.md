---
name: Bug Report
about: Create a report to help us improve
title: '[BUG] '
labels: bug
assignees: ''
---

## Bug Description
A clear and concise description of what the bug is.

## Steps to Reproduce
1.
2.
3.

## Expected Behavior
What you expected to happen.

## Actual Behavior
What actually happened.

## Environment
- Install method: [e.g. `make install`, `cargo run`]
- GPU: [e.g. Tesla P40, local]
- Server models: [e.g. MOSS-Image, ACE-Step, Qwen3]
- OS / distro: [e.g. CachyOS / Arch]

## Relevant files
Paste the matching lines (no secrets, no API keys):
- `output/run-<timestamp>.log` tail (the pipeline log)
- `output/<slug>/meta.json` excerpt (brief, seeds, selection)
- `output/<slug>/work/score.json` worst finding
- `output/<slug>/work/judge/verdict.json` summary (provider, score, must_fix)

## Screenshots/Videos
If applicable, add screenshots or videos to help explain your problem.

## Additional Context
Add any other context about the problem here.
