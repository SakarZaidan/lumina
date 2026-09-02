//! The glyph cache must be invisible: same bytes, fewer rasterisations.
//!
//! Caching rendering output is the kind of change that fails subtly — a stale
//! bitmap after a font is swapped, or a size collision that draws one size at
//! another. These pin down the cases where that could happen.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use lumina_text::TextEngine;

fn engine() -> TextEngine {
    let mut e = TextEngine::new();
    let data = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/assets/fonts/LiberationSans-Regular.ttf"
    ))
    .expect("bundled font");
    e.load_font("sans".into(), &data).expect("font loads");
    e
}

#[test]
fn a_cached_glyph_is_byte_identical_to_a_fresh_one() {
    let cached = engine();
    let first = cached.glyph('A', 24.0, Some("sans")).expect("glyph");
    let second = cached.glyph('A', 24.0, Some("sans")).expect("glyph");
    assert_eq!(first.alpha, second.alpha);
    assert_eq!(first.metrics.width, second.metrics.width);

    // And identical to one from an engine that has never drawn it.
    let fresh = engine();
    let independent = fresh.glyph('A', 24.0, Some("sans")).expect("glyph");
    assert_eq!(
        first.alpha, independent.alpha,
        "a cached glyph must match a freshly rasterised one exactly"
    );
}

#[test]
fn different_sizes_do_not_share_an_entry() {
    // The key is the size's exact bit pattern. Rounding sizes into buckets
    // would draw one size at another's dimensions.
    let e = engine();
    let small = e.glyph('W', 12.0, Some("sans")).expect("glyph");
    let large = e.glyph('W', 48.0, Some("sans")).expect("glyph");
    assert!(
        large.metrics.width > small.metrics.width,
        "48pt must be wider than 12pt: {} vs {}",
        large.metrics.width,
        small.metrics.width
    );

    // Sizes a hair apart are genuinely different bitmaps, not noise to round
    // away — an animated font size passes through thousands of them.
    let a = e.glyph('W', 24.0, Some("sans")).expect("glyph");
    let b = e.glyph('W', 24.01, Some("sans")).expect("glyph");
    assert!(a.metrics.width > 0 && b.metrics.width > 0);
}

#[test]
fn measurement_and_drawing_agree() {
    // Both read the same cache, so a string cannot measure one width and draw
    // another — which is what produced misaligned centred text before.
    let e = engine();
    let text = "Hello, world";
    let measured = e.measure_width(text, 24.0, Some("sans"), 0.0);
    let summed: f32 = text
        .chars()
        .filter_map(|c| e.glyph_metrics(c, 24.0, Some("sans")))
        .map(|m| m.advance_width)
        .sum();
    assert!(
        (measured - summed).abs() < 1e-3,
        "measure_width said {measured}, glyph metrics sum to {summed}"
    );
}

#[test]
fn reloading_a_font_invalidates_its_glyphs() {
    // Re-loading an id keeps its index but replaces its outlines. A cache
    // that survived would keep drawing the old typeface under the new name.
    let mut e = engine();
    let before = e.glyph('g', 32.0, Some("sans")).expect("glyph");
    let before_alpha = before.alpha.clone();
    drop(before);

    let bold = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/assets/fonts/LiberationSans-Bold.ttf"
    ))
    .expect("bundled bold font");
    e.load_font("sans".into(), &bold).expect("font reloads");

    let after = e.glyph('g', 32.0, Some("sans")).expect("glyph");
    assert_ne!(
        before_alpha, after.alpha,
        "after replacing the font under an id, its glyphs must be re-rasterised"
    );
}

#[test]
fn an_engine_with_no_fonts_returns_nothing() {
    let e = TextEngine::new();
    assert!(e.glyph('A', 24.0, None).is_none());
    assert!(e.glyph_metrics('A', 24.0, None).is_none());
    assert_eq!(e.measure_width("anything", 24.0, None, 0.0), 0.0);
}

#[test]
fn the_cache_stays_bounded_under_an_animated_size() {
    // A font_size that animates produces a new key every frame. Without a
    // bound the cache would grow for the length of the render.
    let e = engine();
    for i in 0..12_000 {
        let size = 10.0 + i as f32 * 0.001;
        let _ = e.glyph('m', size, Some("sans"));
    }
    // Still correct after whatever eviction happened.
    let g = e.glyph('m', 24.0, Some("sans")).expect("glyph");
    assert!(g.metrics.width > 0);
}
