###TARGET = x86_64-unknown-linux-musl
###TARGET = aarch64-unknown-linux-musl
TARGET = armv7-unknown-linux-musleabihf
BINARY = klok

.PHONY: all build strip clean size

all: build strip size

build:
	cargo build --release --target $(TARGET)

strip:
	strip target/$(TARGET)/release/$(BINARY)

size:
	ls -lh target/$(TARGET)/release/$(BINARY)

clean:
	cargo clean
