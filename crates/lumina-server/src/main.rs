use axum::{
    extract::Json,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use lumina_export::Exporter;
use lumina_renderer::skia_backend::SkiaRenderer;
use lumina_schema::Scene;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

#[derive(Debug, Serialize, Deserialize)]
struct RenderRequest {
    scene: Scene,
    #[serde(default = "default_format")]
    format: String,
}

fn default_format() -> String {
    "mp4".to_string()
}

#[derive(Debug, Serialize)]
struct ValidationResponse {
    valid: bool,
    errors: Vec<ValidationError>,
}

#[derive(Debug, Serialize)]
struct ValidationError {
    path: String,
    message: String,
    fix_suggestion: Option<String>,
}

async fn validate_scene(Json(scene): Json<Scene>) -> impl IntoResponse {
    // Basic validation is already handled by Serde deserialization.
    // We can add deeper semantic validation here.
    Json(ValidationResponse {
        valid: true,
        errors: vec![],
    })
}

async fn render_scene(Json(payload): Json<RenderRequest>) -> Response {
    let renderer = SkiaRenderer::new();
    let mut exporter = Exporter::new(renderer);
    
    let temp_dir = tempfile::tempdir().unwrap();
    let output_path = temp_dir.path().join("output.mp4");

    match exporter.export_mp4(&payload.scene, &output_path) {
        Ok(_) => {
            let data = std::fs::read(output_path).unwrap();
            Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", "video/mp4")
                .body(axum::body::Body::from(data))
                .unwrap()
        }
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Rendering failed: {}", e)).into_response()
        }
    }
}

async fn health_check() -> &'static str {
    "Lumina Server is online"
}

#[tokio::main]
async fn main() {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/validate", post(validate_scene))
        .route("/render", post(render_scene))
        .layer(CorsLayer::permissive());

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    log::info!("Lumina Server listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
