//! Tests for the command implementations.
//!
//! The CLI measured **0% coverage** while the rest of the workspace sat above
//! 80% (TD-10), for the ordinary reason: everything lived in `main`, and a
//! `main` cannot be called. These test the logic now that it can be.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

fn write_temp(name: &str, contents: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lumina-cli-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let path = dir.join(name);
    std::fs::write(&path, contents).expect("write");
    path
}

#[test]
fn a_parse_error_names_the_file_and_the_position() {
    // A scene is hand-written or model-written JSON, and "expected `,`" with
    // no position is not something either can act on.
    let path = write_temp("broken.lsf", r#"{"version": "1.0","#);
    let err = load_scene(&path).expect_err("must fail");
    let text = err.to_string();
    assert!(text.contains("broken.lsf"), "no file named: {text}");
    assert!(
        text.contains("line") || text.contains("column"),
        "no position given: {text}"
    );
}

#[test]
fn a_missing_file_says_so_rather_than_reporting_a_parse_error() {
    let err = load_scene(Path::new("/nonexistent/scene.lsf")).expect_err("must fail");
    assert!(err.to_string().contains("cannot read"), "{err}");
}

#[test]
fn the_template_renders_and_validates() {
    // A starter template that does not validate is worse than no template.
    // This also pins that it *animates*: a template producing a static frame
    // gives no signal that the toolchain works end to end.
    let text = scene_template("demo");
    let scene: Scene = serde_json::from_str(&text).expect("template is a valid scene");
    let result = validate_scene_data(&scene);
    assert!(
        result.valid,
        "template does not validate: {:?}",
        result.errors
    );
    assert!(!scene.timeline.is_empty(), "the template does not animate");
}

#[test]
fn new_scene_refuses_to_clobber() {
    let path = write_temp("existing.lsf", "{}");
    let err = new_scene(&path).expect_err("must refuse");
    assert!(err.to_string().contains("already exists"), "{err}");
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "{}");
}

#[test]
fn a_valid_scene_reports_ok_and_an_invalid_one_does_not() {
    let good = scene_template("t");
    let scene: Scene = serde_json::from_str(&good).expect("scene");
    let (text, ok) = format_validation(&validate_scene_data(&scene), Report::Human);
    assert!(ok);
    assert!(text.contains("ok"), "{text}");

    let mut broken: serde_json::Value = serde_json::from_str(&good).expect("json");
    broken["canvas"]["fps"] = serde_json::json!(100_000);
    let scene: Scene = serde_json::from_value(broken).expect("scene");
    let (text, ok) = format_validation(&validate_scene_data(&scene), Report::Human);
    assert!(!ok);
    assert!(text.contains("error:"), "{text}");
    assert!(
        text.contains("fix:"),
        "an error must carry its remedy: {text}"
    );
}

#[test]
fn json_output_is_machine_readable() {
    // The point of the JSON mode: a script or an agent reads this, and the
    // human formatting above is prose it would have to parse.
    let scene: Scene = serde_json::from_str(&scene_template("t")).expect("scene");
    let (text, _) = format_validation(&validate_scene_data(&scene), Report::Json);
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
    assert!(parsed["valid"].is_boolean());
    assert!(parsed["errors"].is_array());
}

#[test]
fn the_output_extension_follows_the_format() {
    // `--format mp4 -o out` writing a file called `out` produces something no
    // player opens on a double-click.
    assert_eq!(
        output_path(Path::new("out"), "mp4"),
        PathBuf::from("out.mp4")
    );
    assert_eq!(
        output_path(Path::new("out"), "webm"),
        PathBuf::from("out.webm")
    );
    assert_eq!(
        output_path(Path::new("out"), "mov"),
        PathBuf::from("out.mov")
    );
    assert_eq!(
        output_path(Path::new("out"), "gif"),
        PathBuf::from("out.gif")
    );
}

#[test]
fn an_explicit_extension_is_respected() {
    assert_eq!(
        output_path(Path::new("reel.mkv"), "mp4"),
        PathBuf::from("reel.mkv"),
        "the caller's own extension was overwritten"
    );
}

#[test]
fn frame_sequences_stay_directories() {
    // `png` and `exr` write numbered files into a directory; giving that
    // directory a `.mp4` extension would be nonsense.
    assert_eq!(
        output_path(Path::new("frames"), "png"),
        PathBuf::from("frames")
    );
    assert_eq!(
        output_path(Path::new("frames"), "exr"),
        PathBuf::from("frames")
    );
}

#[test]
fn every_registered_easing_plots() {
    // The plot samples the real easing, so this also exercises every curve in
    // the registry — including the parameterised ones, which fall back to
    // documented defaults when given no params.
    for name in luminafx_core::easing::EASING_NAMES {
        let plot = plot_easing(name, 40, 12).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(plot.contains(name), "the plot does not name the curve");
        assert!(plot.lines().count() >= 12, "{name} plotted too short");
    }
}

#[test]
fn an_overshooting_easing_is_not_clamped_flat() {
    // `ease_out_elastic` overshoots past 1, and that overshoot is the entire
    // reason to choose it. A plot clamped to the unit square would draw it
    // much like `ease_out_cubic`, which is the one thing the plot exists to
    // distinguish.
    let plot = plot_easing("ease_out_elastic", 60, 14).expect("plots");
    let header = plot.lines().next().expect("header");
    let hi: f32 = header
        .rsplit('…')
        .next()
        .and_then(|s| s.trim().trim_end_matches(']').parse().ok())
        .unwrap_or(0.0);
    assert!(hi > 1.0, "the overshoot was flattened away: {header}");
}

#[test]
fn an_unknown_easing_suggests_the_nearest_name() {
    let err = plot_easing("ease_in_cubi", 20, 8).expect_err("must fail");
    let text = err.to_string();
    assert!(
        text.contains("ease_in_cubic"),
        "no suggestion offered: {text}"
    );
}

#[test]
fn the_easing_list_names_every_registered_curve() {
    let list = easing_list();
    for name in luminafx_core::easing::EASING_NAMES {
        assert!(list.contains(name), "{name} is missing from the list");
    }
}
