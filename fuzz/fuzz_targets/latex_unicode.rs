//! Fuzz the LaTeX-to-Unicode transliterator.
//!
//! It reads brace groups, super/subscripts, and fractions by index, so
//! unbalanced or truncated input is the interesting case. It is also reachable
//! straight from a scene file with no other validation in between.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    // Bound the input: the transliterator is O(n) in passes over the string,
    // and the fuzzer is looking for panics, not for slow inputs.
    if text.len() > 4096 {
        return;
    }
    let _ = lumina_renderer::testing::latex::latex_to_unicode(text);
});
