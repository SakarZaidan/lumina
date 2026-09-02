//! Criterion benchmarks for the hot paths: timeline evaluation, a single
//! Skia frame at 1080p, and easing dispatch.
//!
//! Run with `cargo bench -p lumina-bench`.

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
);
criterion_main!(benches);
