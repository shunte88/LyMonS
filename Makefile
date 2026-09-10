# LyMonS Makefile
# Build targets for main binary and plugins

.PHONY: all build plugins install-plugins clean help pcp cross_pi release_pi cross_pi64 release_pi64 cross_pi_armv6 release_pi_armv6 cross_opi release_opi

# Default target
all: build plugins

# Build main LyMonS binary
build:
	@echo "Building LyMonS main binary..."
	cargo build --release

# Build all plugins
plugins:
	@echo "Building plugins..."
	@cd drivers/lymons-driver-ssd1306 && cargo build --release
	@cd drivers/lymons-driver-ssd1309 && cargo build --release
	@cd drivers/lymons-driver-sh1106 && cargo build --release
	@cd drivers/lymons-driver-ssd1322 && cargo build --release
	@mkdir -p target/release/drivers
	@cp target/release/liblymons_driver_ssd1306.so target/release/drivers/
	@cp target/release/liblymons_driver_ssd1309.so target/release/drivers/
	@cp target/release/liblymons_driver_sh1106.so target/release/drivers/
	@cp target/release/liblymons_driver_ssd1322.so target/release/drivers/
	@echo "Plugins built successfully!"
	@echo "Plugin location: target/release/drivers/"
	@ls -lh target/release/drivers/

# Build with workspace (builds everything)
workspace:
	@echo "Building workspace (main + all plugins)..."
	cargo build --release --workspace

# Install plugins to system directories
install-plugins: plugins
	@echo "Installing plugins to /usr/local/lib/lymons/drivers/..."
	@sudo mkdir -p /usr/local/lib/lymons/drivers
	@sudo cp target/release/liblymons_driver_ssd1306.so /usr/local/lib/lymons/drivers/
	@sudo cp target/release/liblymons_driver_ssd1309.so /usr/local/lib/lymons/drivers/
	@sudo cp target/release/liblymons_driver_sh1106.so /usr/local/lib/lymons/drivers/
	@sudo cp target/release/liblymons_driver_ssd1322.so /usr/local/lib/lymons/drivers/
	@echo "Plugins installed successfully!"

# Install plugins to user directory
install-plugins-user: plugins
	@echo "Installing plugins to ~/.local/lib/lymons/drivers/..."
	@mkdir -p ~/.local/lib/lymons/drivers
	@cp target/release/liblymons_driver_ssd1306.so ~/.local/lib/lymons/drivers/
	@cp target/release/liblymons_driver_ssd1309.so ~/.local/lib/lymons/drivers/
	@cp target/release/liblymons_driver_sh1106.so ~/.local/lib/lymons/drivers/
	@cp target/release/liblymons_driver_ssd1322.so ~/.local/lib/lymons/drivers/
	@echo "Plugins installed successfully!"

# Build minimal binary (plugin-only mode)
build-minimal:
	@echo "Building minimal LyMonS binary (plugin-only mode)..."
	cargo build --release --no-default-features --features plugin-only

# Build embedded binary (single static driver, no plugins)
build-embedded:
	@echo "Building embedded LyMonS binary..."
	cargo build --release --no-default-features --features embedded

# Create PiCorePlayer deployment package
pcp: all
	@echo "Creating PiCorePlayer deployment package..."
	@./scripts/create-pcp-package.sh
	@echo ""
	@echo "Package created successfully!"
	@ls -lh lymons-*-pcp.tgz 2>/dev/null || echo "Package file not found"

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	cargo clean
	@echo "Clean complete!"

# Cross-compile for Raspberry Pi (armv6 - Pi 1, Zero and Zero W)
# Rust has no armv6-* triple; arm-unknown-linux-gnueabihf is the ARMv6 build and
# is labelled "armv6" in the package name and release assets.
cross_pi_armv6:
	@echo "Cross-compiling for Raspberry Pi (armv6)..."
	@./scripts/cross-compile-pi.sh arm-unknown-linux-gnueabihf

# Create Raspberry Pi armv6 release package (Pi 1, Zero, Zero W)
release_pi_armv6: cross_pi_armv6
	@echo "Creating Raspberry Pi armv6 deployment package..."
	@./scripts/create-pi-package.sh arm-unknown-linux-gnueabihf
	@echo ""
	@echo "Package created successfully!"
	@ls -lh lymons-*-pi-armv6.tgz 2>/dev/null || echo "Package file not found"

# Cross-compile for Raspberry Pi (armv7 - Pi 2/3/4 on a 32-bit OS)
# Also runs on the 32-bit Allwinner Orange Pi boards (Zero/PC/One, H2+/H3)
cross_pi:
	@echo "Cross-compiling for Raspberry Pi (armv7)..."
	@./scripts/cross-compile-pi.sh armv7-unknown-linux-gnueabihf

# Cross-compile for Raspberry Pi (aarch64 - Pi 3/4/5 and Zero 2 W on a 64-bit OS)
cross_pi64:
	@echo "Cross-compiling for Raspberry Pi (aarch64)..."
	@./scripts/cross-compile-pi.sh aarch64-unknown-linux-gnu

# Create Raspberry Pi release package (armv7 - most compatible)
release_pi: cross_pi
	@echo "Creating Raspberry Pi deployment package..."
	@./scripts/create-pi-package.sh armv7-unknown-linux-gnueabihf
	@echo ""
	@echo "Package created successfully!"
	@ls -lh lymons-*-pi-armv7.tgz 2>/dev/null || echo "Package file not found"

# Create Raspberry Pi 64-bit release package
release_pi64: cross_pi64
	@echo "Creating Raspberry Pi 64-bit deployment package..."
	@./scripts/create-pi-package.sh aarch64-unknown-linux-gnu
	@echo ""
	@echo "Package created successfully!"
	@ls -lh lymons-*-pi-aarch64.tgz 2>/dev/null || echo "Package file not found"

# Cross-compile for Orange Pi (aarch64 - Rockchip RK3588/RK3566/RK3399 and
# Allwinner H618; covers OPi 5 family, 3B, 4, Zero 3 and Zero 2W)
cross_opi:
	@echo "Cross-compiling for Orange Pi (aarch64)..."
	@./scripts/cross-compile-opi.sh aarch64-unknown-linux-gnu

# Create Orange Pi release package
release_opi: cross_opi
	@echo "Creating Orange Pi deployment package..."
	@./scripts/create-opi-package.sh aarch64-unknown-linux-gnu
	@echo ""
	@echo "Package created successfully!"
	@ls -lh lymons-*-opi-aarch64.tgz 2>/dev/null || echo "Package file not found"

# Show help
help:
	@echo "LyMonS Build System"
	@echo ""
	@echo "Targets:"
	@echo "  all                  - Build main binary and plugins (default)"
	@echo "  build                - Build main LyMonS binary"
	@echo "  plugins              - Build all plugins"
	@echo "  workspace            - Build everything using workspace"
	@echo "  pcp                  - Create PiCorePlayer deployment package (.tgz)"
	@echo "  cross_pi_armv6       - Cross-compile for Raspberry Pi (armv6, Pi 1/Zero/Zero W)"
	@echo "  cross_pi             - Cross-compile for Raspberry Pi (armv7 32-bit)"
	@echo "  cross_pi64           - Cross-compile for Raspberry Pi (aarch64 64-bit)"
	@echo "  release_pi_armv6     - Build and package for Raspberry Pi (armv6)"
	@echo "  release_pi           - Build and package for Raspberry Pi (armv7)"
	@echo "  release_pi64         - Build and package for Raspberry Pi (aarch64)"
	@echo "  cross_opi            - Cross-compile for Orange Pi (aarch64, Rockchip + Allwinner)"
	@echo "  release_opi          - Build and package for Orange Pi (aarch64, Rockchip + Allwinner)"
	@echo "  install-plugins      - Install plugins system-wide (requires sudo)"
	@echo "  install-plugins-user - Install plugins to user directory"
	@echo "  build-minimal        - Build minimal binary (plugin-only mode)"
	@echo "  build-embedded       - Build embedded binary (single static driver)"
	@echo "  clean                - Clean build artifacts"
	@echo "  help                 - Show this help message"
	@echo ""
	@echo "Plugin locations:"
	@echo "  Development: ./target/release/drivers/"
	@echo "  User:        ~/.local/lib/lymons/drivers/"
	@echo "  System:      /usr/local/lib/lymons/drivers/"
	@echo ""
	@echo "Cross-compilation:"
	@echo "  Raspberry Pi (armv6):   make release_pi_armv6"
	@echo "  Raspberry Pi (armv7):   make release_pi"
	@echo "  Raspberry Pi (aarch64): make release_pi64"
	@echo "  Orange Pi (aarch64):    make release_opi"
	@echo ""
	@echo "Every display driver is built into every package. The panel, bus"
	@echo "(I2C or SPI) and GPIO controller are chosen at runtime in lymons.yaml."
