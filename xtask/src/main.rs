//! Repository automation.
//!
//! `cargo xtask ci` runs the same checks CI runs, in the same order, so a
//! contributor finds out locally what the merge gate would tell them. It
//! replaces the five commands `CONTRIBUTING.md` used to ask people to retype,
//! and — more importantly — it means the gate is defined in one place. When CI
//! and a README drift apart, the README loses silently.
//!
//! Usage:
//!
//! ```text
//! cargo xtask ci          # everything CI runs
//! cargo xtask ci --fast   # skip wasm, book, and example renders
//! cargo xtask fmt         # format the workspace
//! cargo xtask examples    # render every example scene
//! ```

// This is a developer tool, not library code: failing loudly on a broken
// environment is the correct behaviour, and every panic here is a setup error
// the developer needs to see.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::process::{Command, ExitCode};

/// One step of the gate.
struct Step {
    /// Shown while running and in the summary.
    name: &'static str,
    /// Program to run.
    program: &'static str,
    /// Arguments, verbatim.
    args: Vec<String>,
    /// Environment overrides applied to this step only.
    env: Vec<(&'static str, &'static str)>,
    /// Skipped by `--fast` (slow, or needs a tool not everyone has).
    slow: bool,
    /// A missing tool is reported and skipped rather than failing the run.
    optional: bool,
}

fn step(name: &'static str, program: &'static str, args: &[&str]) -> Step {
    Step {
        name,
        program,
        args: args.iter().map(|s| (*s).to_string()).collect(),
        env: Vec::new(),
        slow: false,
        optional: false,
    }
}

impl Step {
    fn slow(mut self) -> Self {
        self.slow = true;
        self
    }
    fn optional(mut self) -> Self {
        self.optional = true;
        self
    }
    fn env(mut self, key: &'static str, value: &'static str) -> Self {
        self.env.push((key, value));
        self
    }
}

/// The gate, in the order CI runs it: cheapest and most likely to fail first,
/// so a formatting slip does not cost a full test run to discover.
fn ci_steps() -> Vec<Step> {
    vec![
        step("fmt", "cargo", &["fmt", "--all", "--check"]),
        step(
            "clippy",
            "cargo",
            &[
                "clippy",
                "--workspace",
                "--exclude",
                "lumina-wasm",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        ),
        // LUMINA_REQUIRE_VELLO turns "no wgpu adapter" from a silent skip into
        // a failure, so cross-backend parity cannot quietly stop being checked.
        step(
            "test",
            "cargo",
            &[
                "test",
                "--workspace",
                "--exclude",
                "lumina-wasm",
                "--exclude",
                "lumina-bench",
            ],
        )
        .env("LUMINA_REQUIRE_VELLO", "1"),
        step(
            "msrv",
            "cargo",
            &[
                "check",
                "--workspace",
                "--exclude",
                "lumina-wasm",
                "--exclude",
                "lumina-bench",
                "--all-targets",
            ],
        ),
        step(
            "docs",
            "cargo",
            &[
                "doc",
                "--no-deps",
                "--workspace",
                "--exclude",
                "lumina-wasm",
                "--exclude",
                "lumina-bench",
            ],
        ),
        step("deny", "cargo", &["deny", "check"]).optional(),
        step(
            "wasm-build",
            "wasm-pack",
            &["build", "crates/lumina-wasm", "--target", "web"],
        )
        .slow()
        .optional(),
        step(
            "wasm-test",
            "wasm-pack",
            &["test", "--node", "crates/lumina-wasm"],
        )
        .slow()
        .optional(),
        step("book", "mdbook", &["build", "docs"]).slow().optional(),
    ]
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let task = args.first().map(String::as_str).unwrap_or("ci");
    let fast = args.iter().any(|a| a == "--fast");

    match task {
        "ci" => run_ci(fast),
        "fmt" => run_one(&step("fmt", "cargo", &["fmt", "--all"])),
        "examples" => render_examples(args.iter().any(|a| a == "--full")),
        "help" | "--help" | "-h" => {
            print_help();
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("unknown task '{other}'\n");
            print_help();
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    eprintln!(
        "cargo xtask <task>\n\
         \n\
         Tasks:\n\
         \x20 ci [--fast]   Run the merge gate. --fast skips wasm, book, and examples.\n\
         \x20 fmt           Format the workspace.\n\
         \x20 examples [--full]\n\
         \x20               Check every example renders. Draws one frame per scene by\n\
         \x20               default; --full encodes complete videos (slow, needs ffmpeg).\n\
         \x20 help          This message."
    );
}

fn run_ci(fast: bool) -> ExitCode {
    let steps = ci_steps();
    let total = steps.len();
    let mut failed: Vec<&str> = Vec::new();
    let mut skipped: Vec<&str> = Vec::new();

    for (i, s) in steps.iter().enumerate() {
        if fast && s.slow {
            skipped.push(s.name);
            continue;
        }
        eprintln!("\n── [{}/{total}] {} ──", i + 1, s.name);

        let mut cmd = Command::new(s.program);
        cmd.args(&s.args);
        for (k, v) in &s.env {
            cmd.env(k, v);
        }

        match cmd.status() {
            Ok(status) if status.success() => {}
            Ok(_) => failed.push(s.name),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && s.optional => {
                eprintln!("   {} not installed — skipping", s.program);
                skipped.push(s.name);
            }
            Err(e) => {
                eprintln!("   could not run {}: {e}", s.program);
                failed.push(s.name);
            }
        }
    }

    eprintln!("\n──────────────────────────────");
    if !skipped.is_empty() {
        eprintln!("skipped: {}", skipped.join(", "));
    }
    if failed.is_empty() {
        eprintln!("gate passed");
        ExitCode::SUCCESS
    } else {
        eprintln!("gate FAILED: {}", failed.join(", "));
        ExitCode::FAILURE
    }
}

fn run_one(s: &Step) -> ExitCode {
    let mut cmd = Command::new(s.program);
    cmd.args(&s.args);
    match cmd.status() {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

/// Check every example scene still renders.
///
/// `ENGINEERING_PRINCIPLES` #12 says a broken example is a broken build, and
/// until this existed nothing checked it.
///
/// By default it draws **one frame** per scene rather than encoding the whole
/// video. That is a deliberate trade: the flagship showcase is 4 500 frames,
/// and a gate slow enough to be skipped is not a gate. A single frame still
/// exercises everything that actually breaks — the scene parses, validates,
/// every declared asset resolves and decodes, and every object type draws.
/// `--full` encodes complete videos for a release check.
fn render_examples(full: bool) -> ExitCode {
    let dir = std::path::Path::new("examples");
    let Ok(entries) = std::fs::read_dir(dir) else {
        eprintln!("no examples/ directory — run from the repository root");
        return ExitCode::FAILURE;
    };

    let mut scenes: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "lsf"))
        .collect();
    scenes.sort();

    if scenes.is_empty() {
        eprintln!("no .lsf scenes found in examples/");
        return ExitCode::FAILURE;
    }

    eprintln!("building lumina-cli …");
    if !Command::new("cargo")
        .args(["build", "--release", "-p", "lumina-cli"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return ExitCode::FAILURE;
    }

    let out = std::path::Path::new("target/xtask-examples");
    if let Err(e) = std::fs::create_dir_all(out) {
        eprintln!("cannot create {}: {e}", out.display());
        return ExitCode::FAILURE;
    }

    let mut failed = Vec::new();
    for scene in &scenes {
        let name = scene.file_stem().unwrap_or_default().to_string_lossy();
        eprint!("  {name} … ");
        let ext = if full { "mp4" } else { "png" };
        let target = out.join(format!("{name}.{ext}"));

        let mut cmd = Command::new("target/release/lumina-cli");
        cmd.args([
            "--scene",
            &scene.to_string_lossy(),
            "--output",
            &target.to_string_lossy(),
        ]);
        if full {
            cmd.args(["--format", "mp4"]);
        } else {
            cmd.arg("--preview");
        }

        let ok = cmd.status().map(|s| s.success()).unwrap_or(false);
        if ok {
            eprintln!("ok");
        } else {
            eprintln!("FAILED");
            failed.push(name.to_string());
        }
    }

    if failed.is_empty() {
        let mode = if full { "rendered" } else { "drew a frame" };
        eprintln!("\nall {} examples {mode}", scenes.len());
        ExitCode::SUCCESS
    } else {
        eprintln!("\nfailed: {}", failed.join(", "));
        ExitCode::FAILURE
    }
}
