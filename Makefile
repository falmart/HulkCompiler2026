build:
	cargo build --release
	cp target/release/hulkc ./hulk

clean:
	cargo clean
	rm -f ./hulk ./output

.PHONY: build clean
