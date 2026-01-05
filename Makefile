.PHONY: build lint clean

build:
	cargo build --release

lint:
	cargo clippy -- -D warnings

clean:
	cargo clean
