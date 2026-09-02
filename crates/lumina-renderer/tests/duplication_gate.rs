//! WS-02 acceptance gate (TD-02): parsing and geometry logic must live in
//! `src/common/` exactly once. This test fails if telltale parser code
//! reappears in either backend file.

use std::path::Path;

#[test]
fn backends_contain_no_duplicated_parser_code() {
    // Signature strings of the logic that was deduplicated. Each entry is a
    // fragment that can only appear if that decision has been reimplemented in
    // a backend rather than consumed from `common/`.
    let forbidden = [
        // Hex colour parsing.
        "from_str_radix",
        // The per-variant z-index match.
        "=> p.z_index",
        // Plot sampling: where to evaluate a function is a rendering decision,
        // and it was previously written twice — including the substring
        // rewrite that turned `asin(` into `amath::sin(`.
        "math::sin(",
        "eval_number_with_context",
        // Tick positions. `t += step` accumulates error; the `+ 1e-4` fudge
        // that used to guard the loop was the symptom.
        "while t <= end",
        // Glyph placement (TD-18). Where a glyph goes is layout and belongs in
        // `common::text`; the backends may only differ in how they composite
        // it. This fragment is the fontdue baseline arithmetic that was
        // written out in both files.
        "metrics.ymin as f32",
    ];
    for file in ["src/skia_backend.rs", "src/vello_backend.rs"] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(file);
        let src =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path:?}: {e}"));
        for needle in forbidden {
            assert!(
                !src.contains(needle),
                "{file} contains `{needle}` — that logic belongs in src/common/ \
                 (one implementation shared by both backends, see TD-02)"
            );
        }
    }
}
