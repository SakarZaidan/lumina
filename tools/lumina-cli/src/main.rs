//! Command-line renderer for Lumina scenes.
//!
//! Loads an LSF scene file and renders it to a PNG frame sequence, MP4, `WebM`,
//! or GIF on either the CPU (`skia`) or GPU (`vello`) backend:
//!
//! ```text
//! lumina-cli --scene examples/hello.lsf --output hello --format mp4
//! lumina-cli --scene scene.lsf --backend vello --format webm
//! lumina-cli --scene scene.lsf --watch   # live PNG preview on file change
//! ```
//!
//! Video formats require `ffmpeg` on PATH (see `lumina-export`).

// The engine has never contained `unsafe`, and the metric tracking that was a
// `grep` over the source — which by v0.4.0 was returning a false positive from
// the word appearing in a comment. `forbid` makes it a compile error instead:
// it cannot be silenced by an `allow` further down, so a future `unsafe` block
// has to be argued for by removing this line, in a diff a reviewer will see.
#![forbid(unsafe_code)]

use anyhow::Context;
use clap::Parser;
use lumina_export::{AudioTrack, Exporter, Quality};
use lumina_renderer::{skia_backend::SkiaRenderer, vello_backend::VelloRenderer, Renderer};
use lumina_schema::Scene;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Lumina animation engine CLI — JSON in, video out.",
    long_about = None
)]
struct Args {
    /// Path to the LSF scene file (.lsf or .json)
    #[arg(short, long)]
    scene: PathBuf,

    /// Output file or directory
    #[arg(short, long, default_value = "output")]
    output: PathBuf,

    /// Output format: png (frame sequence), mp4, webm, webm-alpha, mov, or gif
    ///
    /// `webm-alpha` (VP9) and `mov` (`ProRes` 4444) carry an alpha channel, for
    /// compositing a scene with a transparent `canvas.background` over other
    /// footage.
    #[arg(short, long, default_value = "png")]
    format: String,

    /// Renderer backend: skia (CPU, default) or vello (GPU)
    #[arg(short, long, default_value = "skia")]
    backend: String,

    /// Watch mode: re-render on every file change (outputs a single PNG preview)
    #[arg(short, long)]
    watch: bool,

    /// Print per-frame timing at the end of a render
    #[arg(long)]
    verbose: bool,

    /// Encoding quality: draft (fast), standard (default), or final (10-bit).
    ///
    /// Affects encoder effort and bit depth only — rendering is identical at
    /// every setting, so a draft render and a final render show the same
    /// pixels before compression.
    #[arg(long, default_value = "standard")]
    quality: String,

    /// Render a single frame to PNG instead of the full animation.
    ///
    /// Takes an optional time in seconds; defaults to the midpoint. Useful for
    /// checking a scene loads and draws without paying for every frame — which
    /// is how CI verifies every example still renders.
    #[arg(long, value_name = "SECONDS", num_args = 0..=1, default_missing_value = "")]
    preview: Option<String>,

    /// Validate the scene and exit without rendering (exit code 1 on errors)
    #[arg(long)]
    check: bool,
}

fn load_scene(path: &PathBuf) -> anyhow::Result<Scene> {
    let text =
        std::fs::read_to_string(path).map_err(|e| anyhow::anyhow!("Cannot read {path:?}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| {
        // Surface line/col in parse errors for better DX
        anyhow::anyhow!("Scene parse error in {path:?}: {e}")
    })
}

/// Run semantic validation, print every finding, and fail on errors.
/// Warnings (duplicate keyframes, missing easing params, …) never block.
fn validate_scene(scene: &Scene, path: &PathBuf) -> anyhow::Result<()> {
    let result = lumina_core::validation::validate_scene_data(scene);
    for w in &result.warnings {
        eprintln!("[warn]  {} at {}: {}", w.code, w.path, w.message);
    }
    for e in &result.errors {
        eprintln!(
            "[error] {} at {}: {}\n        fix: {}",
            e.code, e.path, e.message, e.fix_suggestion
        );
    }
    if !result.valid {
        anyhow::bail!(
            "{:?} failed validation with {} error(s)",
            path,
            result.errors.len()
        );
    }
    Ok(())
}

fn render_once(args: &Args, scene: &Scene) -> anyhow::Result<Vec<Duration>> {
    let timings = match args.backend.as_str() {
        "skia" => {
            let mut renderer = SkiaRenderer::new();
            load_fonts(&mut renderer, scene)?;
            load_images(&mut renderer, scene)?;
            let mut exporter = Exporter::new(renderer);
            do_export(&mut exporter, scene, args)?
        }
        "vello" => {
            let mut renderer = VelloRenderer::new()?;
            load_fonts(&mut renderer, scene)?;
            load_images(&mut renderer, scene)?;
            let mut exporter = Exporter::new(renderer);
            do_export(&mut exporter, scene, args)?
        }
        other => anyhow::bail!("Unknown backend '{other}'. Valid: skia, vello"),
    };
    Ok(timings)
}

fn load_fonts<R: Renderer>(renderer: &mut R, scene: &Scene) -> anyhow::Result<()> {
    for font_asset in &scene.assets.fonts {
        log::info!(
            "Loading font '{}' from {:?}",
            font_asset.id,
            font_asset.path
        );
        let data = std::fs::read(&font_asset.path)
            .map_err(|e| anyhow::anyhow!("Cannot read font {:?}: {}", font_asset.path, e))?;
        renderer
            .load_font(&font_asset.id, &data)
            .map_err(|e| anyhow::anyhow!("Font load error: {e:?}"))?;
    }
    Ok(())
}

fn load_images<R: Renderer>(renderer: &mut R, scene: &Scene) -> anyhow::Result<()> {
    for image_asset in &scene.assets.images {
        log::info!(
            "Loading image '{}' from {:?}",
            image_asset.id,
            image_asset.path
        );
        let data = std::fs::read(&image_asset.path)
            .map_err(|e| anyhow::anyhow!("Cannot read image {:?}: {}", image_asset.path, e))?;
        renderer
            .load_image(&image_asset.id, &data)
            .map_err(|e| anyhow::anyhow!("Image load error: {e:?}"))?;
    }
    Ok(())
}

fn do_export<R: Renderer>(
    exporter: &mut Exporter<R>,
    scene: &Scene,
    args: &Args,
) -> anyhow::Result<Vec<Duration>> {
    let mut timings = Vec::new();

    // Audio paths are resolved here rather than inside the exporter, which
    // never sees the scene's own path strings. The CLI's trust boundary is the
    // working directory, so a relative path is taken at face value exactly as
    // fonts and images are; the server resolves the same assets against
    // `LUMINA_ASSET_ROOT` instead.
    let tracks: Vec<AudioTrack> = scene
        .assets
        .audio
        .iter()
        .map(|a| AudioTrack::new(std::path::PathBuf::from(&a.path), a))
        .collect();
    for track in &tracks {
        if !track.path.exists() {
            anyhow::bail!("audio asset not found at {}", track.path.display());
        }
    }
    exporter.set_audio(tracks);

    let quality = match args.quality.as_str() {
        "draft" => Quality::Draft,
        "standard" => Quality::Standard,
        "final" => Quality::Final,
        other => anyhow::bail!("unknown --quality '{other}'; expected draft, standard, or final"),
    };
    match args.format.as_str() {
        "mp4" => {
            log::info!("Exporting MP4 to {:?}", args.output);
            let t0 = Instant::now();
            exporter.export_mp4_with(scene, &args.output, quality)?;
            timings.push(t0.elapsed());
        }
        "webm" => {
            log::info!("Exporting WebM to {:?}", args.output);
            let t0 = Instant::now();
            exporter.export_webm_with(scene, &args.output, quality)?;
            timings.push(t0.elapsed());
        }
        "webm-alpha" => {
            log::info!("Exporting WebM with alpha to {:?}", args.output);
            let t0 = Instant::now();
            exporter.export_webm_alpha_with(scene, &args.output, quality)?;
            timings.push(t0.elapsed());
        }
        "mov" | "prores" => {
            log::info!("Exporting ProRes 4444 MOV to {:?}", args.output);
            let t0 = Instant::now();
            exporter.export_mov_prores4444_with(scene, &args.output, quality)?;
            timings.push(t0.elapsed());
        }
        "gif" => {
            log::info!("Exporting GIF to {:?}", args.output);
            let t0 = Instant::now();
            exporter.export_gif(scene, &args.output)?;
            timings.push(t0.elapsed());
        }
        "png" => {
            log::info!("Exporting PNG sequence to {:?}", args.output);
            let t0 = Instant::now();
            exporter.export_png_sequence(scene, &args.output)?;
            timings.push(t0.elapsed());
        }
        other => {
            anyhow::bail!("Unknown format '{other}'. Valid: png, mp4, webm, webm-alpha, mov, gif")
        }
    }
    Ok(timings)
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args = Args::parse();

    if args.watch {
        run_watch(&args)
    } else {
        let scene = load_scene(&args.scene)?;
        validate_scene(&scene, &args.scene)?;
        if args.check {
            println!("[lumina] {:?} is valid.", args.scene);
            return Ok(());
        }
        if let Some(spec) = &args.preview {
            let time = if spec.is_empty() {
                scene.canvas.duration / 2.0
            } else {
                spec.parse::<f32>().map_err(|_| {
                    anyhow::anyhow!("--preview expects a time in seconds, got '{spec}'")
                })?
            };
            render_preview(&scene, &args.output, time, &args.backend)?;
            println!(
                "[lumina] preview of {:?} at t={time:.2}s written to {:?}",
                args.scene, args.output
            );
            return Ok(());
        }
        log::info!(
            "Rendering '{}' with {} backend…",
            scene.meta.title,
            args.backend
        );
        let timings = render_once(&args, &scene)?;
        if args.verbose {
            println!("[lumina] render complete in {:.2?}", timings[0]);
        }
        log::info!("Done.");
        Ok(())
    }
}

/// Watch mode: render on start, then re-render whenever the scene file changes.
/// The output is always a single PNG frame (the mid-point of the scene) so
/// previews are near-instant even for long animations.
fn run_watch(args: &Args) -> anyhow::Result<()> {
    use notify::{RecursiveMode, Watcher};

    let preview_out = args.output.with_extension("png");
    println!(
        "[watch] Watching {:?} — preview → {:?}",
        args.scene, preview_out
    );

    let do_preview = |path: &PathBuf| {
        let t0 = Instant::now();
        let scene = match load_scene(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[watch] parse error: {e}");
                return;
            }
        };
        if let Err(e) = validate_scene(&scene, path) {
            eprintln!("[watch] {e}");
            return;
        }
        // Render mid-point frame to a single PNG
        let mid = scene.canvas.duration / 2.0;
        match render_preview(&scene, &preview_out, mid, &args.backend) {
            Ok(_) => println!(
                "[watch] preview updated in {:.0?}  {:?}",
                t0.elapsed(),
                preview_out
            ),
            Err(e) => eprintln!("[watch] render error: {e}"),
        }
    };

    // Initial render
    do_preview(&args.scene);

    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res| {
        if let Ok(event) = res {
            let _ = tx.send(event);
        }
    })?;
    watcher.watch(&args.scene, RecursiveMode::NonRecursive)?;

    println!(
        "[watch] ready — edit {:?} to trigger a re-render. Ctrl-C to quit.",
        args.scene
    );
    for _event in rx {
        do_preview(&args.scene);
    }
    Ok(())
}

/// Load every font and image the scene declares, failing on the first problem.
///
/// Preview rendering used to do this with `if let Ok(data) = … { let _ = … }`,
/// so a missing or corrupt asset produced a frame with no text and **no
/// message** — while the full render path hard-errored on exactly the same
/// input. Two behaviours for one fault, and the quiet one is the default in
/// watch mode, where a typo'd path looks like a styling problem.
fn load_declared_assets<R: lumina_renderer::Renderer>(
    renderer: &mut R,
    scene: &Scene,
) -> anyhow::Result<()> {
    for fa in &scene.assets.fonts {
        let data = std::fs::read(&fa.path)
            .with_context(|| format!("cannot read font '{}' at {}", fa.id, fa.path))?;
        renderer
            .load_font(&fa.id, &data)
            .map_err(|e| anyhow::anyhow!("cannot load font '{}': {e:?}", fa.id))?;
    }
    for ia in &scene.assets.images {
        let data = std::fs::read(&ia.path)
            .with_context(|| format!("cannot read image '{}' at {}", ia.id, ia.path))?;
        renderer
            .load_image(&ia.id, &data)
            .map_err(|e| anyhow::anyhow!("cannot load image '{}': {e:?}", ia.id))?;
    }
    Ok(())
}

fn render_preview(scene: &Scene, output: &PathBuf, time: f32, backend: &str) -> anyhow::Result<()> {
    use image::{ImageBuffer, Rgba};
    use lumina_core::{SceneGraph, Timeline};

    let scene_graph = SceneGraph::from_scene(scene);
    let timeline = Timeline::from_scene(scene);
    let states = timeline.get_state_at(time);
    let cam_state = timeline.get_camera_at(time, scene);
    let camera = scene.camera.as_ref().map(|_| &cam_state);

    let frame = match backend {
        "vello" => {
            let mut r = VelloRenderer::new()?;
            load_declared_assets(&mut r, scene)?;
            lumina_renderer::Renderer::set_time(&mut r, time);
            lumina_renderer::Renderer::render_frame(
                &mut r,
                &scene_graph.objects,
                &states,
                scene.canvas.width,
                scene.canvas.height,
                &scene.canvas.background,
                camera,
            )
            .map_err(|e| anyhow::anyhow!("{e:?}"))?
        }
        _ => {
            let mut r = SkiaRenderer::new();
            load_declared_assets(&mut r, scene)?;
            lumina_renderer::Renderer::set_time(&mut r, time);
            lumina_renderer::Renderer::render_frame(
                &mut r,
                &scene_graph.objects,
                &states,
                scene.canvas.width,
                scene.canvas.height,
                &scene.canvas.background,
                camera,
            )
            .map_err(|e| anyhow::anyhow!("{e:?}"))?
        }
    };

    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(scene.canvas.width, scene.canvas.height, frame)
            .ok_or_else(|| anyhow::anyhow!("Failed to build image from frame data"))?;
    img.save(output)?;
    Ok(())
}
