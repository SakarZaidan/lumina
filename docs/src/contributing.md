# Contributing

Contributions are welcome. The bar is simple: the workspace stays green and new
behavior is covered by a test.

## Before opening a PR

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --exclude lumina-wasm --exclude lumina-bench
cargo build -p lumina-wasm            # wasm lib compiles
```

For the SDKs:

```bash
(cd sdks/python && maturin develop && python -c "import lumina; print(lumina.__version__)")
(cd sdks/javascript && npm install && npm run build)
```

## Conventions

- Every new schema field is `#[serde(default)]` so existing `.lsf` files keep working.
- New object types must be handled in: the schema enum, the Skia `z_index` and draw match, the Vello match, and the WASM `hit_test`/`get_z_index`.
- Add a renderer or unit test for any rendering change; pixel assertions over a known scene are preferred.
- Update `CHANGELOG.md` (`[Unreleased]`) and the README roadmap when a feature is user-facing.

## Building the docs

```bash
cargo install mdbook
mdbook serve docs        # live preview at http://localhost:3000
```
