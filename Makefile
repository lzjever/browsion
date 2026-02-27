# Use scripts/cargo so cargo works in Cursor's terminal (ARGV0 workaround).
CARGO := $(shell pwd)/scripts/cargo

.PHONY: build test build-tauri test-tauri

build: build-tauri
build-tauri:
	cd src-tauri && $(CARGO) build

test: test-tauri
test-tauri:
	cd src-tauri && $(CARGO) test
