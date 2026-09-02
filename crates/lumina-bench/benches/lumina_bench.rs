//! Criterion benchmarks for the engine's hot paths.
//!
//! Run with `cargo bench -p lumina-bench`.
//!
//! Every group here exists because some specific change needs a number to
//! justify it (`ENGINEERING_PRINCIPLES` #5). The original three groups —
//! timeline evaluation, a Skia frame, easing dispatch — could not surface the
//! largest costs in the engine: nothing drew text, nothing plotted a function,
//! and nothing ran an export. A benchmark suite that cannot see the problem is
//! not a regression gate.

// Benchmarks are not `#[cfg(test)]` items, so clippy.toml's allow-in-tests
// does not reach them. Setup failure here should panic, not be handled.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(missing_docs)]

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use lumina_core::{SceneGraph, Timeline};
use lumina_renderer::{skia_backend::SkiaRenderer, Renderer};
use lumina_schema::{Canvas, CircleProps, Meta, Object, Scene, TimelineEntry};
use std::collections::HashMap;

// ── Scene factories ───────────────────────────────────────────────────────────

fn make_scene(n_objects: usize) -> Scene {
    let mut objects = HashMap::new();
    let mut timeline = Vec::new();

    for i in 0..n_objects {
        let id = format!("c{i}");
        objects.insert(
            id.clone(),
            Object::Circle(CircleProps {
                cx: (i % 100) as f32 * 10.0,
                cy: (i / 100) as f32 * 10.0,
                radius: 5.0,
                z_index: i as i32,
                fill: "#FF6B6B".into(),
                stroke: None,
                stroke_width: 0.0,
                shadow: None,
                opacity: 1.0,
            }),
        );
        // Two keyframes per object
        timeline.push(TimelineEntry {
            time: 0.0,
            object: id.clone(),
            state: serde_json::json!({ "cx": 0.0, "opacity": 0.0 }),
            easing: "ease_out_cubic".into(),
            easing_params: None,
        });
        timeline.push(TimelineEntry {
            time: 2.0,
            object: id,
            state: serde_json::json!({ "cx": 200.0, "opacity": 1.0 }),
            easing: "ease_out_cubic".into(),
            easing_params: None,
        });
    }

    Scene {
        version: "1.0".into(),
        meta: Meta {
            title: "Bench".into(),
            author: "bench".into(),
            created_at: "2026-01-01".into(),
        },
        canvas: Canvas {
            width: 1920,
            height: 1080,
            fps: 30,
            duration: 4.0,
            background: "#0F0F1A".into(),
        },
        assets: Default::default(),
        objects,
        timeline,
        events: vec![],
        camera: None,
    }
}

// ── Benchmarks ────────────────────────────────────────────────────────────────

fn bench_timeline_evaluation(c: &mut Criterion) {
    let mut group = c.benchmark_group("timeline_eval");
    for n in [100usize, 500, 1000, 2000] {
        let scene = make_scene(n);
        let timeline = Timeline::from_scene(&scene);
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let states = timeline.get_state_at(black_box(1.0));
                black_box(states);
            })
        });
    }
    group.finish();
}

fn bench_skia_frame_render(c: &mut Criterion) {
    let mut group = c.benchmark_group("skia_render");
    for n in [10usize, 100, 500] {
        let scene = make_scene(n);
        let scene_graph = SceneGraph::from_scene(&scene);
        let timeline = Timeline::from_scene(&scene);
        let states = timeline.get_state_at(1.0);
        let mut renderer = SkiaRenderer::new();

        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let pixels = renderer
                    .render_frame(&scene_graph.objects, &states, 1920, 1080, "#0F0F1A", None)
                    .unwrap();
                black_box(pixels);
            })
        });
    }
    group.finish();
}

/// A scene whose cost is dominated by text.
///
/// Glyphs are rasterised from outlines on every frame with no cache, and
/// `font_for_char` linearly walks every loaded font per character. Nothing in
/// the original suite drew a single character, so the largest single cost in
/// the engine was invisible to it.
fn make_text_scene(n_labels: usize, chars_per_label: usize) -> Scene {
    use lumina_schema::TextProps;

    let mut objects = HashMap::new();
    let content: String = "The quick brown fox jumps over the lazy dog "
        .chars()
        .cycle()
        .take(chars_per_label)
        .collect();

    for i in 0..n_labels {
        objects.insert(
            format!("t{i}"),
            Object::Text(TextProps {
                content: content.clone(),
                x: 40.0,
                y: 40.0 + (i % 40) as f32 * 25.0,
                font_size: 24.0,
                color: "#E8E8F0".into(),
                font_id: Some("sans".into()),
                z_index: i as i32,
                opacity: 1.0,
                align: "left".into(),
                letter_spacing: 0.0,
            }),
        );
    }

    let mut scene = make_scene(0);
    scene.objects = objects;
    scene.timeline = Vec::new();
    scene
}

/// A scene dominated by plotted functions.
fn make_plot_scene(n_plots: usize, samples: u32) -> Scene {
    use lumina_schema::{AxesProps, PlotProps};

    let mut objects = HashMap::new();
    objects.insert(
        "ax".to_string(),
        Object::Axes(AxesProps {
            x_range: [-10.0, 10.0],
            y_range: [-5.0, 5.0],
            x: 100.0,
            y: 540.0,
            scale: 40.0,
            x_step: 1.0,
            y_step: 1.0,
            x_label: None,
            y_label: None,
            grid: true,
            z_index: 0,
            color: "#4A9EFF".into(),
            opacity: 1.0,
        }),
    );
    for i in 0..n_plots {
        objects.insert(
            format!("p{i}"),
            Object::Plot(PlotProps {
                function_str: format!("math::sin({} * x) / {}", i + 1, i + 1),
                axes_id: "ax".into(),
                color: "#FFD93D".into(),
                stroke_width: 2.0,
                sample_count: samples,
                z_index: (i + 1) as i32,
                draw_fraction: None,
                opacity: 1.0,
            }),
        );
    }

    let mut scene = make_scene(0);
    scene.objects = objects;
    scene.timeline = Vec::new();
    scene
}

/// The bundled OFL font, so text benchmarks measure real glyph rasterisation
/// rather than a fallback that draws nothing.
fn load_bench_font(renderer: &mut SkiaRenderer) {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/assets/fonts/LiberationSans-Regular.ttf"
    );
    let data = std::fs::read(path).expect("bundled font must be present");
    renderer
        .load_font("sans", &data)
        .expect("bundled font must load");
}

/// Text rendering: the path with no cache anywhere (`AAA-P-01`).
fn bench_text_render(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_render");
    // Fewer samples than the default: each iteration rasterises thousands of
    // glyphs, so the default 100 would make the suite unpleasant to run.
    group.sample_size(20);

    for (labels, chars) in [(10usize, 40usize), (40, 40)] {
        let scene = make_text_scene(labels, chars);
        let graph = SceneGraph::from_scene(&scene);
        let timeline = Timeline::from_scene(&scene);
        let states = timeline.get_state_at(0.0);
        let mut renderer = SkiaRenderer::new();
        load_bench_font(&mut renderer);

        group.bench_function(
            BenchmarkId::from_parameter(format!("{labels}x{chars}")),
            |b| {
                b.iter(|| {
                    black_box(
                        renderer
                            .render_frame(
                                &graph.objects,
                                &states,
                                1920,
                                1080,
                                &scene.canvas.background,
                                None,
                            )
                            .expect("render"),
                    )
                })
            },
        );
    }
    group.finish();
}

/// Plot rendering: expression evaluation and adaptive sampling (`AAA-P-09`).
fn bench_plot_render(c: &mut Criterion) {
    let mut group = c.benchmark_group("plot_render");
    group.sample_size(20);

    for (plots, samples) in [(1usize, 200u32), (8, 200), (8, 2000)] {
        let scene = make_plot_scene(plots, samples);
        let graph = SceneGraph::from_scene(&scene);
        let timeline = Timeline::from_scene(&scene);
        let states = timeline.get_state_at(0.0);
        let mut renderer = SkiaRenderer::new();

        group.bench_function(
            BenchmarkId::from_parameter(format!("{plots}plots_{samples}samples")),
            |b| {
                b.iter(|| {
                    black_box(
                        renderer
                            .render_frame(
                                &graph.objects,
                                &states,
                                1920,
                                1080,
                                &scene.canvas.background,
                                None,
                            )
                            .expect("render"),
                    )
                })
            },
        );
    }
    group.finish();
}

/// A whole render pass: evaluate the timeline and draw, once per frame.
///
/// This is what export wall-time is actually made of, minus the encode. The
/// per-frame `Pixmap` allocation and the `to_vec` copy that the `Renderer`
/// trait forces (`AAA-P-02`) only show up when frames are rendered in
/// sequence, which no other group does.
fn bench_frame_sequence(c: &mut Criterion) {
    let mut group = c.benchmark_group("frame_sequence");
    group.sample_size(10);

    let scene = make_scene(100);
    let graph = SceneGraph::from_scene(&scene);
    let timeline = Timeline::from_scene(&scene);
    let mut renderer = SkiaRenderer::new();

    for frames in [30usize, 120] {
        group.bench_function(
            BenchmarkId::from_parameter(format!("{frames}frames")),
            |b| {
                b.iter(|| {
                    for i in 0..frames {
                        let t = i as f32 / 30.0;
                        let states = timeline.get_state_at(t);
                        black_box(
                            renderer
                                .render_frame(
                                    &graph.objects,
                                    &states,
                                    1280,
                                    720,
                                    &scene.canvas.background,
                                    None,
                                )
                                .expect("render"),
                        );
                    }
                })
            },
        );
    }
    group.finish();
}

/// Scene-graph ordering, recomputed on every frame today (`AAA-P-06`).
fn bench_scene_walk(c: &mut Criterion) {
    let mut group = c.benchmark_group("scene_walk");

    for n in [100usize, 1000] {
        let scene = make_scene(n);
        let graph = SceneGraph::from_scene(&scene);
        group.bench_function(BenchmarkId::from_parameter(n), |b| {
            b.iter(|| black_box(SceneGraph::from_scene(black_box(&scene))));
        });
        // Timeline construction is per-render, not per-frame, but it is on the
        // critical path for a preview and worth watching.
        group.bench_function(
            BenchmarkId::from_parameter(format!("timeline_build_{n}")),
            |b| {
                b.iter(|| black_box(Timeline::from_scene(black_box(&scene))));
            },
        );
        black_box(&graph);
    }
    group.finish();
}

/// LaTeX rendering: `latex_to_unicode` runs ~70 chained `String::replace`
/// passes over input that never changes, per object, per frame (`AAA-P-08`).
/// Nothing in the suite measured it, so nothing could say whether memoising it
/// was worth the cache.
fn bench_latex_render(c: &mut Criterion) {
    use lumina_schema::LaTeXProps;

    let mut group = c.benchmark_group("latex_render");
    group.sample_size(20);

    for n in [1usize, 20] {
        let mut objects = HashMap::new();
        for i in 0..n {
            objects.insert(
                format!("l{i}"),
                Object::LaTeX(LaTeXProps {
                    expression:
                        r"\sum_{n=1}^{\infty} \frac{1}{n^2} = \frac{\pi^2}{6} \quad \alpha\beta\gamma"
                            .into(),
                    x: 40.0,
                    y: 40.0 + (i % 30) as f32 * 30.0,
                    font_size: 28.0,
                    color: "#E8E8F0".into(),
                    z_index: i as i32,
                    opacity: 1.0,
                    draw_fraction: None,
                    align: "left".into(),
                    letter_spacing: 0.0,
                }),
            );
        }
        let mut scene = make_scene(0);
        scene.objects = objects;
        scene.timeline = Vec::new();

        let graph = SceneGraph::from_scene(&scene);
        let states = Timeline::from_scene(&scene).get_state_at(0.0);
        let mut renderer = SkiaRenderer::new();
        load_bench_font(&mut renderer);

        group.bench_function(BenchmarkId::from_parameter(format!("{n}formulas")), |b| {
            b.iter(|| {
                black_box(
                    renderer
                        .render_frame(&graph.objects, &states, 1920, 1080, "#0F0F1A", None)
                        .expect("render"),
                )
            })
        });
    }
    group.finish();
}

fn bench_easing_dispatch(c: &mut Criterion) {
    use lumina_core::easing::{eval_easing, get_easing_fn};

    let mut group = c.benchmark_group("easing");

    group.bench_function("get_easing_fn_lookup", |b| {
        b.iter(|| {
            let f = get_easing_fn(black_box("ease_out_elastic"));
            black_box(f(0.5_f32));
        })
    });

    group.bench_function("eval_easing_cubic_bezier", |b| {
        let params = serde_json::json!([0.25, 0.1, 0.25, 1.0]);
        b.iter(|| {
            black_box(eval_easing(
                black_box("cubic_bezier"),
                Some(&params),
                black_box(0.5_f32),
            ))
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_timeline_evaluation,
    bench_skia_frame_render,
    bench_easing_dispatch,
    bench_text_render,
    bench_plot_render,
    bench_frame_sequence,
    bench_scene_walk,
    bench_latex_render,
);
criterion_main!(benches);
