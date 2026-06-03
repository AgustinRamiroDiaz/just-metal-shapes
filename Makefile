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

.PHONY: help check check-web-exporter rust-web web-export web-build web-serve web-run clean-web

help:
	@printf '%s\n' \
		'Targets:' \
		'  make check       - Check the native Rust extension build' \
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
