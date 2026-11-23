---
name: Pull Request
about: Create a pull request
title: ''
labels: ''
assignees: ''
---

## Description
A clear and concise description of the changes in this PR.

## Type of Change
- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] Documentation update
- [ ] Code refactoring
- [ ] Performance improvement
- [ ] Test addition or update
- [ ] CI/CD update

## Area
- [ ] Pipeline and stages (`src/pipeline.rs`, `src/media.rs`)
- [ ] Score, retry and judge (`src/score.rs`, `src/retry.rs`, `src/judge.rs`)
- [ ] History and dashboard (`src/history.rs`, `src/dashboard.rs`, `dashboard/`)
- [ ] Video assembly (`src/video.rs`, `video/`)
- [ ] NES chip (`src/chip.rs`, `src/chip/`)
- [ ] Reference library (`src/refs/`, `src/references.rs`)
- [ ] Docs, RFCs, skills or CI

## Checklist
- [ ] My code follows the style guidelines of this project
- [ ] I have performed a self-review of my own code
- [ ] I have commented my code, particularly in hard-to-understand areas
- [ ] I have made corresponding changes to the documentation
- [ ] `make check` passes locally (fmt, build, clippy, tests, skills)
- [ ] No files outside the change are reformatted
- [ ] Any dependent changes have been merged and published

## Screenshots/Videos
If applicable, add screenshots or videos to help explain your changes.
