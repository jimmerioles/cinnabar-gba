.PHONY: build run demo debug selftest builtins input celeste bench test lint fmt validate smoke soak wasm roms demo-rom clean

CARGO ?= cargo
ROM ?=

build:
	$(CARGO) build --release --workspace

run:
	$(CARGO) run -p cinnabar-frontend -- $(if $(ROM),$(ROM),--demo)

demo:
	$(CARGO) run -p cinnabar-frontend -- --demo

debug:
	$(CARGO) run -p cinnabar-frontend -- $(if $(ROM),$(ROM),--demo) --debug

smoke:
	./tools/smoke.sh

soak:
	./tools/soak.sh $(ROM)

wasm:
	wasm-pack build crates/cinnabar-wasm --target web --out-dir web/pkg

selftest:
	$(CARGO) run -p cinnabar-frontend -- --selftest

builtins:
	$(CARGO) run -p cinnabar-frontend -- --list-builtins

input:
	$(CARGO) run -p cinnabar-frontend -- --builtin input

celeste:
	./tools/fetch-celeste.sh
	$(CARGO) run -p cinnabar-frontend -- test-roms/homebrew/celeste.gba

bench:
	$(CARGO) run --release -p cinnabar-core --example bench -- $(if $(ROM),$(ROM),test-roms/homebrew/celeste.gba) 300 1

test:
	$(CARGO) test --workspace

lint:
	$(CARGO) clippy --workspace --all-targets -- -D warnings

fmt:
	$(CARGO) fmt --all

roms:
	./tools/fetch-test-roms.sh

validate: roms
	./tools/validate.sh

demo-rom:
	./tools/build-demo-rom.sh

clean:
	$(CARGO) clean
