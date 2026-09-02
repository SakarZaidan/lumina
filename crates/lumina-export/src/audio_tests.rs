//! A scene declares an audio asset and the video carries it (`AAA-OUT-08`).
//!
//! The interesting claims are about *length* and *placement*, because those
//! are where a plausible-looking ffmpeg command line silently does the wrong
//! thing: `-shortest` alone truncates the video to a short track, `apad` alone
//! extends it to fill a long one, and `amix` normalises by input count unless
//! told not to, so declaring a second track halves the first.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use luminafx_renderer::skia_backend::SkiaRenderer;
use luminafx_schema::Scene;

use crate::{AudioTrack, Exporter};

fn ffmpeg_available() -> bool {
    std::process::Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lumina-audio-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    dir
}

/// A two-second 440 Hz tone, so the fixture does not need a binary asset.
fn make_tone(dir: &Path, seconds: f32) -> PathBuf {
    let path = dir.join("tone.wav");
    let status = std::process::Command::new("ffmpeg")
        .args(["-y", "-hide_banner", "-loglevel", "error", "-f", "lavfi"])
        .arg("-i")
        .arg(format!("sine=frequency=440:duration={seconds}"))
        .arg(&path)
        .status()
        .expect("ffmpeg");
    assert!(status.success(), "could not synthesise a test tone");
    path
}

/// A one-second scene: shorter than a two-second tone, longer than a
/// half-second one, so both directions of the length question are testable.
fn scene(duration: f32) -> Scene {
    serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "meta": { "title": "audio", "author": "t", "created_at": "2026-09-02T00:00:00Z" },
        "canvas": { "width": 32, "height": 32, "fps": 10, "duration": duration,
                    "background": "#101018" },
        "objects": {
            "r": { "type": "Rectangle",
                   "properties": { "x": 4, "y": 4, "width": 24, "height": 24,
                                   "fill": "#4DABF7", "z_index": 1 } }
        },
        "timeline": []
    }))
    .expect("scene")
}

/// Duration of a stream, in seconds, as the container reports it.
fn stream_duration(path: &Path, kind: &str) -> f64 {
    let out = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            kind,
            "-show_entries",
            "stream=duration",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .expect("ffprobe");
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .unwrap_or(-1.0)
}

fn has_audio_stream(path: &Path) -> bool {
    let out = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "a",
            "-show_entries",
            "stream=codec_type",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .expect("ffprobe");
    String::from_utf8_lossy(&out.stdout).contains("audio")
}

fn export_with_tone(dir: &Path, video_secs: f32, tone_secs: f32, start: f32) -> PathBuf {
    let tone = make_tone(dir, tone_secs);
    let out = dir.join("out.mp4");
    let mut exporter = Exporter::new(SkiaRenderer::new());
    exporter.set_audio(vec![AudioTrack {
        path: tone,
        start,
        gain: 1.0,
    }]);
    exporter
        .export_mp4(&scene(video_secs), &out)
        .expect("export with audio");
    out
}

#[test]
fn a_declared_track_reaches_the_file() {
    if !ffmpeg_available() {
        return;
    }
    let dir = tmp("basic");
    let out = export_with_tone(&dir, 1.0, 1.0, 0.0);
    assert!(has_audio_stream(&out), "the MP4 carries no audio stream");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_long_track_does_not_extend_the_video() {
    // `apad` pads with silence indefinitely; without `-shortest` alongside it
    // the encode never terminates on its own, and with neither, a two-second
    // track stretches a one-second animation to two.
    if !ffmpeg_available() {
        return;
    }
    let dir = tmp("long");
    let out = export_with_tone(&dir, 1.0, 3.0, 0.0);
    let v = stream_duration(&out, "v:0");
    assert!(
        (v - 1.0).abs() < 0.25,
        "a 3s track turned a 1s animation into {v}s of video"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_short_track_does_not_truncate_the_video() {
    // The mirror image, and the one `-shortest` alone gets wrong: a half-second
    // track would cut a two-second animation down to half a second.
    if !ffmpeg_available() {
        return;
    }
    let dir = tmp("short");
    let out = export_with_tone(&dir, 2.0, 0.4, 0.0);
    let v = stream_duration(&out, "v:0");
    assert!(
        v > 1.7,
        "a 0.4s track truncated a 2s animation to {v}s of video"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_started_track_still_fills_the_whole_video() {
    // A delayed track plus padding must still cover the animation's length
    // rather than leaving the tail unencoded.
    if !ffmpeg_available() {
        return;
    }
    let dir = tmp("delay");
    let out = export_with_tone(&dir, 2.0, 0.5, 1.0);
    assert!(has_audio_stream(&out));
    let v = stream_duration(&out, "v:0");
    assert!((v - 2.0).abs() < 0.25, "video is {v}s, expected about 2");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_scene_with_no_audio_produces_exactly_what_it_did_before() {
    // Every scene that exists declares no audio, and none of them may grow a
    // silent track, change length, or fail to encode.
    if !ffmpeg_available() {
        return;
    }
    let dir = tmp("none");
    let out = dir.join("out.mp4");
    Exporter::new(SkiaRenderer::new())
        .export_mp4(&scene(1.0), &out)
        .expect("export");
    assert!(
        !has_audio_stream(&out),
        "a scene declaring no audio grew an audio stream"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn gif_export_ignores_audio_rather_than_failing() {
    // GIF has no audio track. Passing the maps and codec anyway makes ffmpeg
    // exit non-zero, so a scene with sound would simply fail to export as GIF.
    if !ffmpeg_available() {
        return;
    }
    let dir = tmp("gif");
    let tone = make_tone(&dir, 1.0);
    let out = dir.join("out.gif");
    let mut exporter = Exporter::new(SkiaRenderer::new());
    exporter.set_audio(vec![AudioTrack {
        path: tone,
        start: 0.0,
        gain: 1.0,
    }]);
    exporter.export_gif(&scene(1.0), &out).expect("gif export");
    assert!(out.exists());
    let _ = std::fs::remove_dir_all(&dir);
}
