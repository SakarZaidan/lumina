# Contributing

Contributions are welcome. The bar is simple: the workspace stays green and new
behavior is covered by a test.

The canonical, always-current guide lives in the repository:
**[CONTRIBUTING.md](https://github.com/SakarZaidan/lumina/blob/main/CONTRIBUTING.md)** —
setup, the pre-PR verification gate, commit conventions, and code standards.

The short version:

```bash
cargo fmt --all --check
cargo clippy --workspace --exclude lumina-wasm --all-targets -- -D warnings
cargo test --workspace --exclude lumina-wasm --exclude lumina-bench
```

Rendering changes need a pixel-level test, new schema fields must be
`#[serde(default)]`, and user-facing changes update `CHANGELOG.md` and the
relevant chapter of this book.

## Building this book

```bash
cargo install mdbook
mdbook serve docs        # live preview at http://localhost:3000
```
