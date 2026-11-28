SHELL := /bin/bash
CARGO ?= cargo
SSH_HOST ?= root@192.168.88.38
REMOTE_BIN_DIR ?= /home/ubuntu/bin
REMOTE_CONFIG_DIR ?= /home/ubuntu
LOG_DIR ?= output
PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
BIN := target/release/melodias

# Rust stack overlay (CI-safe base commands, --locked). Provides FMT_CMD,
# BUILD_CMD, LINT_CMD, TEST_CMD and DOCS_CMD used by check below.
-include Makefile.stack

.PHONY: help run generate image judge score youtube history export dashboard site site-videos render video short backfill refs build check skills-check docs-check fmt test install uninstall deploy status log clean

help:
	@printf '%s\n' \
	  'melodias-studio' \
	  '' \
	  '  make run                             TUI' \
	  '  make generate URL=... VIBE="..."     headless pipeline, log in output/' \
	  '  make install                         builds and installs ~/.local/bin/melodias' \
	  '  make image DIR=output/<slug>         regenerate the thumbnail of a finished run' \
	  '  make judge DIR=output/<slug>         review a finished run with the judge' \
  '  make score DIR=output/<slug>         score a finished run, no GPU' \
  '  make youtube DIR=output/<slug>       YouTube copy for a finished run, no GPU' \
  '  make history [ARGS=findings]         read the local history database' \
  '  make export CSV=... OUT=...          static dashboard overview, no server' \
  '  make dashboard                       serve the read-only dashboard on 127.0.0.1' \
  '  make site                            preview the project page on 127.0.0.1:4000' \
  '  make site-videos                     refresh More from the channel from YouTube' \
  '  make backfill [ARGS=--replace]       score the runs in output/ into the history DB' \
  '  make refs ARGS="fetch --source opengameart --concept castle"  fill the reference library' \
  '  make refs ARGS="generate --concept volcano --count 5"  generate references with sd-server' \
  '  make refs ARGS="import --concept castle --license CC0-1.0 pack.zip"  import packs' \
	  '  make render DIR=output/<slug>        produced video with Remotion' \
	  '  make video DIR=output/<slug>         fast video with ffmpeg' \
	  '  make short DIR=output/<slug>         vertical TikTok cut with ffmpeg' \
	  '  make uninstall                       removes the installed binary' \
	  '  make log                             last lines of the newest log' \
	  '  make check                           fmt + build + clippy + tests + skills + docs' \
	  '  make skills-check                    validate the agent skills' \
	  '  make docs-check                      validate make targets cited in docs' \
	  '  make status                          state of llm, sd and audio on the VM' \
	  '  make deploy                          copy deploy/ to the VM' \
	  '  make clean                           cargo clean'

run:
	$(CARGO) run

generate:
	@test -n "$(URL)" || { echo 'missing URL, for example: make generate URL="https://www.youtube.com/watch?v=..." VIBE="epic 8-bit orchestral finale"'; exit 2; }
	@test -n "$(VIBE)" || { echo 'missing VIBE="..."'; exit 2; }
	@mkdir -p "$(LOG_DIR)"
	@$(CARGO) build --release
	@log="$(LOG_DIR)/run-$$(date +%Y%m%d-%H%M%S).log"; \
	  MELODIAS_LOG="$$log" $(BIN) "$(URL)" "$(VIBE)"

build:
	$(CARGO) build

image:
	@test -n "$(DIR)" || { echo 'missing DIR, for example: make image DIR=output/<slug>'; exit 2; }
	$(CARGO) run --release -- image "$(DIR)"

judge:
	@test -n "$(DIR)" || { echo 'missing DIR, for example: make judge DIR=output/<slug>'; exit 2; }
	$(CARGO) run --release -- judge "$(DIR)"

score:
	@test -n "$(DIR)" || { echo 'missing DIR, for example: make score DIR=output/<slug>'; exit 2; }
	$(CARGO) run --release -- score "$(DIR)"

youtube:
	@test -n "$(DIR)" || { echo 'missing DIR, for example: make youtube DIR=output/<slug>'; exit 2; }
	$(CARGO) run --release -- youtube "$(DIR)" $(ARGS)

backfill:
	$(CARGO) run --release -- backfill $(ARGS)

refs:
	@test -n "$(ARGS)" || { echo 'missing ARGS, for example: make refs ARGS="fetch --source opengameart --concept castle --count 20"'; exit 2; }
	$(CARGO) run --release -- refs $(ARGS)

site-videos:
	bash scripts/update-site-videos.sh

history:
	$(CARGO) run --release -- history $(ARGS)

export:
	@test -n "$(CSV)" || { echo 'missing CSV, for example: make export CSV=/tmp/runs.csv OUT=/tmp/overview'; exit 2; }
	@test -n "$(OUT)" || { echo 'missing OUT=...'; exit 2; }
	$(CARGO) run --release -- export "$(CSV)" "$(OUT)" $(ARGS)

dashboard:
	$(CARGO) run --release -- dashboard $(ARGS)

site:
	@bash scripts/prepare-site.sh
	@printf 'site: http://127.0.0.1:4000/ (ctrl-c to stop)\n'
	@python3 -m http.server 4000 --directory site

render:
	@test -n "$(DIR)" || { echo 'missing DIR, for example: make render DIR=output/<slug>'; exit 2; }
	$(CARGO) run --release -- render "$(DIR)"

video:
	@test -n "$(DIR)" || { echo 'missing DIR, for example: make video DIR=output/<slug>'; exit 2; }
	$(CARGO) run --release -- video "$(DIR)"

short:
	@test -n "$(DIR)" || { echo 'missing DIR, for example: make short DIR=output/<slug> [ARGS="--sec 32 --platform tiktok"]'; exit 2; }
	$(CARGO) run --release -- short "$(DIR)" $(ARGS)

check:
	$(FMT_CMD)
	$(BUILD_CMD)
	$(LINT_CMD)
	$(TEST_CMD)
	./scripts/check-skills.sh
	$(DOCS_CMD)

skills-check:
	./scripts/check-skills.sh

docs-check:
	./scripts/check-docs.sh

fmt:
	$(CARGO) fmt

test:
	$(CARGO) test

install:
	$(CARGO) build --release
	@mkdir -p "$(BINDIR)"
	install -m 755 "$(BIN)" "$(BINDIR)/melodias"
	@echo "installed $(BINDIR)/melodias"

uninstall:
	rm -f "$(BINDIR)/melodias"
	@echo "removed $(BINDIR)/melodias"

deploy:
	ssh $(SSH_HOST) 'mkdir -p $(REMOTE_BIN_DIR)'
	scp deploy/env.sh deploy/serve-*.sh $(SSH_HOST):$(REMOTE_BIN_DIR)/
	scp deploy/audio-server.json $(SSH_HOST):$(REMOTE_CONFIG_DIR)/
	ssh $(SSH_HOST) 'chmod +x $(REMOTE_BIN_DIR)/serve-*.sh'

status:
	@ssh $(SSH_HOST) 'for s in llm sd audio; do printf "%-6s " "$$s"; $(REMOTE_BIN_DIR)/serve-$$s.sh status 2>&1 | tail -1; done'

log:
	@latest=$$(ls -t $(LOG_DIR)/*.log 2>/dev/null | head -1); \
	  if [ -z "$$latest" ]; then echo "no logs in $(LOG_DIR)"; else echo "== $$latest =="; tail -n 40 "$$latest"; fi

clean:
	$(CARGO) clean
