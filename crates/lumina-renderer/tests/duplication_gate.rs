//! WS-02 acceptance gate (TD-02): parsing and geometry logic must live in
//! `src/common/` exactly once. This test fails if telltale parser code
//! reappears in either backend file.

use std::path::Path;

#[test]
fn backends_contain_no_duplicated_parser_code() {
    // Signature strings of the logic that was deduplicated: hex color
    // parsing, and the per-variant z-index match.
    let forbidden = ["from_str_radix", "=> p.z_index"];
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
