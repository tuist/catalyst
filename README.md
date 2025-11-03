# Catalyst ⚗️

> **⚠️ Experimental Project**
> This is an experimental tool exploring Tuist → Bazel conversion. Use at your own risk and expect breaking changes.

Convert [Tuist](https://tuist.dev) projects to [Bazel](https://bazel.build) builds.

## Quick Start ⚡

```bash
# Install with Mise
mise use -g ubi:tuist/catalyst

# Run it
catalyst run
```

## Features ✨

- 🔄 **Automatic conversion** from Tuist to Bazel
- 🏗️ **Build with Bazel** using rules_apple
- 📱 **Run in Simulator** with one command
- 💾 **XDG-compliant caching** for graph metadata
- 🎯 **Actual source paths** from Tuist graph

## Installation 📦

### Option 1: Using Mise (Recommended)

```bash
mise use -g ubi:tuist/catalyst
```

### Option 2: From Source

```bash
# Install dependencies
mise install

# Build catalyst
cargo build --release

# Optional: Install globally
./install.sh
```

## Usage 🚀

### Build with Bazel

```bash
catalyst build
# or just
catalyst
```

### Run in Simulator

```bash
catalyst run

# With options
catalyst run --simulator "iPhone 15 Pro"
catalyst run --target myapp
```

## How It Works 🔧

1. **Tuist Graph** - Runs `tuist graph --format json` to extract project structure
2. **Generate Bazel Files** - Creates `WORKSPACE`, `BUILD`, and `.bazelrc`
3. **Build** - Executes `bazel build` with rules_apple
4. **Run** (optional) - Installs and launches app in iOS Simulator

## Tools 🛠️

Managed by Mise (see `mise.toml`):
- Tuist: 4.97.1
- Rust: latest
- Bazel: latest

## Advanced: rules_xcodeproj 🎨

The generated `WORKSPACE` includes commented configuration for [rules_xcodeproj](https://github.com/MobileNativeFoundation/rules_xcodeproj):

- Generates Xcode projects from Bazel targets
- Full IDE support (indexing, debugging, etc.)
- Builds still use Bazel under the hood

Uncomment the section in `WORKSPACE` and update versions to enable.

## Development 👩‍💻

```bash
# Build
cargo build

# Format
cargo fmt

# Lint
cargo clippy

# Test
./build.sh
```

## License

MIT
