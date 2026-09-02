//! Fuzz SVG path parsing.
//!
//! Path data arrives inside scene files and inside SVG assets, both of which
//! may be untrusted. The parser walks a token stream with an index, which is
//! exactly the shape that produces out-of-bounds panics on truncated input.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    // Returning None for unparseable input is fine; panicking is not.
    let _ = luminafx_renderer::testing::path::parse_svg_path(text);
});
