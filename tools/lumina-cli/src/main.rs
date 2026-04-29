use clap::Parser;
use lumina_export::Exporter;
use lumina_renderer::{Renderer, skia_backend::SkiaRenderer};
use lumina_schema::Scene;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the LSF scene file
    #[arg(short, long)]
    scene: PathBuf,

    /// Output file or directory
    #[arg(short, long, default_value = "output")]
    output: PathBuf,

    /// Output format (png or mp4)
    #[arg(short, long, default_value = "png")]
    format: String,

    /// Render backend (vello or skia)
    #[arg(short, long, default_value = "skia")]
    backend: String,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args = Args::parse();

    log::info!("Loading scene from {:?}", args.scene);
    let scene_str = std::fs::read_to_string(&args.scene)?;
    let scene: Scene = serde_json::from_str(&scene_str)?;

    log::info!("Initializing {:?} renderer", args.backend);
    match args.backend.as_str() {
        "skia" => {
            let mut renderer = SkiaRenderer::new();
            
            // Load fonts
            for font_asset in &scene.assets.fonts {
                log::info!("Loading font: {} from {:?}", font_asset.id, font_asset.path);
                let font_data = std::fs::read(&font_asset.path)?;
                renderer.load_font(&font_asset.id, &font_data).map_err(|e| anyhow::anyhow!("{:?}", e))?;
            }

            let mut exporter = Exporter::new(renderer);
            if args.format == "mp4" {
                log::info!("Exporting MP4 to {:?}", args.output);
                exporter.export_mp4(&scene, &args.output)?;
            } else {
                log::info!("Exporting PNG sequence to {:?}", args.output);
                exporter.export_png_sequence(&scene, &args.output)?;
            }
        }
        "vello" => {
            anyhow::bail!("Vello backend not yet implemented in CLI");
        }
        _ => {
            anyhow::bail!("Unknown backend: {}", args.backend);
        }
    }

    log::info!("Done!");
    Ok(())
}
