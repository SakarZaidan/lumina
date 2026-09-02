//! Fuzz plot expression handling.
//!
//! `function_str` comes from the scene, so it is untrusted, and it reaches a
//! recursive-descent parser. Both the namespace rewriting and the sampler must
//! survive arbitrary input.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    // Matches the validation bound on function_str, so the fuzzer explores
    // inputs the engine would actually accept rather than ones it rejects.
    if text.len() > 4096 {
        return;
    }
    let _ = luminafx_renderer::testing::plot::normalize_math_calls(text);
    // A small budget: this is looking for panics, not for slow expressions.
    let _ = luminafx_renderer::testing::plot::sample(text, -10.0, 10.0, -10.0, 10.0, 64);
});
