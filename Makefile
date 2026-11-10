.PHONY: build run demo test lint fmt validate roms demo-rom clean

CARGO ?= cargo
ROM ?=

build:
	$(CARGO) build --release --workspace

run:
	$(CARGO) run -p cinnabar-frontend -- $(if $(ROM),$(ROM),--demo)

demo:
	$(CARGO) run -p cinnabar-frontend -- --demo

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
