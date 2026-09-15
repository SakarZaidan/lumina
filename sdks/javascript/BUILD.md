# Building the JS SDK

```bash
npm install
npm run build      # builds the wasm module first, then bundles
npm pack           # `prepack` runs `build`, so the tarball always has both
```

## Two things that bite, and why the scripts look like this

**`wasm/` is generated, never committed.** The sources import
`'../wasm/luminafx_wasm'`, and nothing used to put anything there — so a clean
checkout could not build at all (TD-12). `build:wasm` produces it, and `build`
depends on it.

**`wasm-pack` writes a `.gitignore` containing `*` into its output directory.**
npm honours `.gitignore` when packing, so that one line silently excluded the
entire `wasm/` folder from the tarball — publishing a package with none of the
WebAssembly it exists to deliver, while `npm pack` reported success. `npm pack`
listing only `dist/` and `package.json` is the symptom. The build script deletes
that file, along with the nested `package.json` wasm-pack also emits, which npm
would otherwise treat as a separate package and skip.

Neither failure is visible from a type-check or a test. The only thing that
catches them is packing the tarball and looking inside it, which CI now does.
