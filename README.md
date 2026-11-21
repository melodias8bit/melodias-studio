![melodias-studio](docs/assets/banner.png)

# melodias-studio

A Rust TUI that generates 8-bit thumbnails, original three to four minute instrumental chiptunes, a 1080p video for a YouTube channel and the upload description, using local models.

## Quick Start

Requires Rust 1.85+, `yt-dlp`, `ffmpeg`, `ffprobe` and three local HTTP services. See [setup](docs/setup.md) for the service contract and an example deployment.

```bash
git clone https://github.com/melodias8bit/melodias-studio.git
cd melodias-studio
make install
melodias
```

## Docs

| Topic | Where |
| --- | --- |
| Requirements, install and server scripts | [docs/setup.md](docs/setup.md) |
| TUI, headless run, output and screenshots | [docs/usage.md](docs/usage.md) |
| Local history dashboard | [dashboard/README.md](dashboard/README.md) |
| Environment, `config.toml` and lifecycle commands | [docs/configuration.md](docs/configuration.md) |
| Pipeline, diagram and design notes | [docs/architecture/README.md](docs/architecture/README.md) |
| Design documents (RFCs) | [docs/rfcs/README.md](docs/rfcs/README.md) |
| Adversary review and video assembly | [docs/rfcs/0001-adversary-review.md](docs/rfcs/0001-adversary-review.md) |
| Roadmap | [docs/roadmap.md](docs/roadmap.md) |
| Project rules for agents | [AGENTS.md](AGENTS.md) |
| Commands | [Makefile](Makefile) |

## Contributing

PRs welcome. Run `make check` before opening one.

## License

MIT. The license field lives in [`Cargo.toml`](Cargo.toml).
