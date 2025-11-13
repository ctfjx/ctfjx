BINARY_NAME := ctfjx
BIN_DIR     := bin
VERSION      = devel
BUILD_FLAGS := -ldflags="-s -w -X github.com/lattesec/ctfjx/version.Version=$(VERSION)"

# Binaries
CTFX  := ctfjx
CTFXD := ctfjxd
WEB   := ctfjx-web
AGENT := ctfjx-agent

ifeq ($(OS),Windows_NT)
RM_CMD:=rd /s /q
NULL:=/dev/nul
EXT:=.exe
else
RM_CMD:=rm -rf
NULL:=/dev/null
EXT=
endif



# =================================== DEFAULT =================================== #

default: all

## default: Runs build and test
.PHONY: default
all: build test

# =================================== HELPERS =================================== #

## help: print this help message
.PHONY: help
help:
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Run "make fmt" and "make lint" before committing code.'
	@echo ''
	@echo 'Commands:'
	@echo 'build  Builds the code.'
	@echo 'test   Test the code.'
	@echo 'deps   Install dependencies.'
	@echo ''
	@echo 'dev   Runs the whole infra in dev.'
	@echo 'prod  Runs the whole infra.'
	@echo ''
	@echo 'fmt      Formats code.'
	@echo 'fmt/ci   Checks formatting..'
	@echo 'lint     Lints code.'
	@echo 'lint/ci  Checks linting.'
	@echo ''
	@echo 'Extra:'
	@echo 'docs  Runs docs.'
	@sed -n 's/^### //p' $(MAKEFILE_LIST) | column -t -s ':' |  sed -e 's/^/ /'




.PHONY: build
build:
	cargo build

.PHONY: test
test:
	cargo test

.PHONY: security
security:
	cargo install cargo-audit
	cargo audit

.PHONY: deps
deps:
	rustup component add clippy
	rustup component add rustfmt




.PHONY: dev
dev:

.PHONY: prod
prod:




.PHONY: fmt
fmt:
	cargo fmt

.PHONY: fmt/ci
fmt/ci:
	cargo fmt --check

.PHONY: lint
lint:
	cargo clippy --fix

.PHONY: lint/ci
lint/ci:
	cargo clippy




.PHONY: docs
docs:
	mkdocs serve -f www/mkdocs.yml -a 0.0.0.0:8000

.PHONY: docs/build
docs/build:
	mkdocs build -f www/mkdocs.yml
