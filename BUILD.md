# Building and Distributing luckybird

## Prerequisites

- Rust toolchain (stable, 2024 edition) — install via [rustup](https://rustup.rs)
- Windows: MSVC build tools or MinGW
- Linux/macOS: standard C toolchain (`gcc`/`clang`)

## Development build

```sh
cargo build -p lb-app
./target/debug/luckybird [FILE...]
```

## Release build

```sh
cargo build --release -p lb-app
./target/release/luckybird [FILE...]   # ~0.9 MB, self-contained
```

Release profile is configured in the root `Cargo.toml`:
- `lto = "fat"` — whole-program link-time optimisation
- `codegen-units = 1` — single codegen unit for maximum optimisation
- `panic = "abort"` — no unwinding machinery
- `strip = "symbols"` — strips debug symbols from the binary

## Running

```sh
luckybird                        # scratch buffer
luckybird src/main.rs            # open a file
luckybird file1.rs file2.rs      # open multiple files
luckybird --headless             # no TUI (smoke test / scripting)
luckybird --version
luckybird --help
```

## Tests

```sh
cargo test --workspace           # all 82 tests
cargo test -p lb-core            # single crate
```

## Distribution

The binary is fully self-contained:
and there are no other runtime shared-library dependencies beyond the C runtime.

### Windows — MSI

```powershell
cargo install cargo-wix
cargo wix init        # generates wix/main.wxs (run once)
cargo wix             # produces target/wix/luckybird-<ver>-x86_64.msi
```

### Linux — Debian/Ubuntu

```sh
cargo install cargo-deb
cargo deb -p lb-app   # produces target/debian/luckybird_<ver>_amd64.deb
```

### Linux — RPM (Fedora/RHEL)

```sh
cargo install cargo-generate-rpm
cargo build --release -p lb-app
cargo generate-rpm -p crates/lb-app
```

### Linux — static binary (musl)

Produces a zero-dependency binary that runs on any Linux distribution.

```sh
cargo install cross
cross build --release --target x86_64-unknown-linux-musl -p lb-app
# output: target/x86_64-unknown-linux-musl/release/luckybird
```

### macOS — app bundle

```sh
cargo install cargo-bundle
cargo bundle --release -p lb-app
```

### Cross-compilation

Use [`cross`](https://github.com/cross-rs/cross) to build any target from a single host:

```sh
cross build --release --target x86_64-pc-windows-gnu   -p lb-app
cross build --release --target x86_64-unknown-linux-musl -p lb-app
cross build --release --target aarch64-apple-darwin     -p lb-app
```

### GitHub Releases (CI)

```yaml
# .github/workflows/release.yml
jobs:
  build:
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-musl
          - os: windows-latest
            target: x86_64-pc-windows-msvc
          - os: macos-latest
            target: aarch64-apple-darwin
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - run: cargo build --release --target ${{ matrix.target }} -p lb-app
      - uses: softprops/action-gh-release@v1
        with:
          files: target/${{ matrix.target }}/release/luckybird*
```
