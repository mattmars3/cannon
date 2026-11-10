.PHONY: build

build:
	cargo build --release --bins

install:
	cargo install --path . --bins

clean:
	rm -r target
