SHELL := /bin/bash

GODOT_BIN ?= godot
STANDARD_GODOT_BIN ?= /tmp/godot-4.6.2-standard/Godot_v4.6.2-stable_linux.x86_64
GODOT_EXPORT_BIN ?= $(if $(wildcard $(STANDARD_GODOT_BIN)),$(STANDARD_GODOT_BIN),$(GODOT_BIN))
GODOT_PROJECT ?= godot
RUST_CRATE ?= rust
RUST_NIGHTLY ?= nightly
EMSDK ?= /tmp/emsdk
WEB_OUT ?= build/web
WEB_PORT ?= 8060
E2E_LOG ?= /tmp/just-metal-shapes-e2e.log
# Run a single e2e scenario: make e2e ONLY=rewind
ONLY ?=

.PHONY: help check build test e2e test-all analyze-music check-web-exporter rust-web web-export web-build web-serve web-run clean-web

help:
	@printf '%s\n' \
		'Targets:' \
		'  make check       - Check the native Rust extension build' \
		'  make build       - Build the native Rust extension (debug)' \
		'  make test        - Rust unit tests (cargo test)' \
		'  make e2e         - Headless Godot e2e scenarios (ONLY=name for one)' \
		'  make test-all    - test + e2e' \
		'  make analyze-music - Regenerate stale godot/music/*.analysis.json' \
		'  make rust-web    - Build the no-thread Rust WASM GDExtension' \
		'  make web-export  - Export the Godot Web build' \
		'  make web-build   - Run rust-web and web-export' \
		'  make web-serve   - Serve build/web locally' \
		'  make web-run     - Build, export, and serve locally' \
		'  make clean-web   - Remove generated web export files' \
		'' \
		'Useful overrides:' \
		'  GODOT_EXPORT_BIN=/path/to/non-mono-godot' \
		'  STANDARD_GODOT_BIN=/path/to/non-mono-godot' \
		'  EMSDK=/path/to/emsdk' \
		'  WEB_PORT=8060'

check:
	cd $(RUST_CRATE) && cargo check

build:
	cd $(RUST_CRATE) && cargo build

test:
	cd $(RUST_CRATE) && cargo test

# Imports first so a fresh checkout has its .godot/ cache, then fails on a non-zero exit
# or on any Rust panic reported through Godot's output.
e2e: build
	"$(GODOT_BIN)" --headless --path "$(GODOT_PROJECT)" --import >/dev/null 2>&1 || true
	set -o pipefail; \
		"$(GODOT_BIN)" --headless --path "$(GODOT_PROJECT)" -s res://tests/run_e2e.gd \
			$(if $(ONLY),-- --only=$(ONLY)) 2>&1 | tee "$(E2E_LOG)"
	@if grep -q "\[panic" "$(E2E_LOG)"; then echo "e2e: Rust panic in Godot output"; exit 1; fi

test-all: test e2e

analyze-music:
	uv run --project devtools devtools/analyze_all.py

check-web-exporter:
	@version="$$("$(GODOT_EXPORT_BIN)" --version)"; \
	case "$$version" in \
		*mono*) \
			printf '%s\n' 'Web export needs a non-Mono/non-.NET Godot binary.'; \
			printf '%s\n' "Current GODOT_EXPORT_BIN=$(GODOT_EXPORT_BIN) reports: $$version"; \
			printf '%s\n' 'Run with GODOT_EXPORT_BIN=/path/to/Godot_v4.6.2-stable_linux.x86_64'; \
			exit 1; \
			;; \
	esac

rust-web: check-web-exporter
	test -f "$(EMSDK)/emsdk_env.sh"
	EMSDK_QUIET=1 source "$(EMSDK)/emsdk_env.sh" >/dev/null && \
		cd $(RUST_CRATE) && \
		GDRUST_GODOT_BIN="$(GODOT_EXPORT_BIN)" \
		cargo +$(RUST_NIGHTLY) build --features nothreads -Zbuild-std --target wasm32-unknown-emscripten

web-export: check-web-exporter
	mkdir -p "$(WEB_OUT)"
	"$(GODOT_EXPORT_BIN)" --headless --path "$(GODOT_PROJECT)" --export-debug Web "../$(WEB_OUT)/index.html"

web-build: rust-web web-export

web-serve:
	cd "$(WEB_OUT)" && python3 -m http.server "$(WEB_PORT)"

web-run: web-build web-serve

clean-web:
	rm -rf "$(WEB_OUT)"
