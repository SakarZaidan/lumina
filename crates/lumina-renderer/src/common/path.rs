//! Backend-neutral path geometry.
//!
//! Geometry is *constructed* here once and adapted to each backend's path
//! type, so both backends fill/stroke identical curves — the foundation of
//! pixel parity. Adding a command here requires updating both adapters.

/// One path command, in absolute canvas coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PathCmd {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    QuadTo(f32, f32, f32, f32),
    CubicTo(f32, f32, f32, f32, f32, f32),
    Close,
}

/// Build a rounded-rectangle path using quadratic corner arcs. Radii are
/// clamped to half the rectangle's extent; zero radii yield sharp corners.
pub(crate) fn rounded_rect(x: f32, y: f32, w: f32, h: f32, rx: f32, ry: f32) -> PathData {
    let rx = rx.min(w / 2.0).max(0.0);
    let ry = ry.min(h / 2.0).max(0.0);
    PathData(vec![
        PathCmd::MoveTo(x + rx, y),
        PathCmd::LineTo(x + w - rx, y),
        PathCmd::QuadTo(x + w, y, x + w, y + ry),
        PathCmd::LineTo(x + w, y + h - ry),
        PathCmd::QuadTo(x + w, y + h, x + w - rx, y + h),
        PathCmd::LineTo(x + rx, y + h),
        PathCmd::QuadTo(x, y + h, x, y + h - ry),
        PathCmd::LineTo(x, y + ry),
        PathCmd::QuadTo(x, y, x + rx, y),
        PathCmd::Close,
    ])
}

/// A backend-neutral path: an ordered list of absolute commands.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct PathData(pub(crate) Vec<PathCmd>);

/// Control-point bounding box `(x, y, w, h)` of a path — the same
/// conservative bounds tiny-skia reports for the equivalent `Path`, so
/// gradient geometry derived from it matches across backends.
pub(crate) fn bbox(p: &PathData) -> Option<(f32, f32, f32, f32)> {
    let mut min = (f32::INFINITY, f32::INFINITY);
    let mut max = (f32::NEG_INFINITY, f32::NEG_INFINITY);
    let mut add = |x: f32, y: f32| {
        min.0 = min.0.min(x);
        min.1 = min.1.min(y);
        max.0 = max.0.max(x);
        max.1 = max.1.max(y);
    };
    for cmd in &p.0 {
        match *cmd {
            PathCmd::MoveTo(x, y) | PathCmd::LineTo(x, y) => add(x, y),
            PathCmd::QuadTo(cx, cy, x, y) => {
                add(cx, cy);
                add(x, y);
            }
            PathCmd::CubicTo(x1, y1, x2, y2, x, y) => {
                add(x1, y1);
                add(x2, y2);
                add(x, y);
            }
            PathCmd::Close => {}
        }
    }
    if min.0.is_finite() {
        Some((min.0, min.1, max.0 - min.0, max.1 - min.1))
    } else {
        None
    }
}

/// Parse SVG path data into a [`PathData`].
/// Supports: M/m (move), L/l (line), H/h (horizontal), V/v (vertical),
///           C/c (cubic bezier), Z/z (close).
pub(crate) fn parse_svg_path(d: &str) -> Option<PathData> {
    let mut cmds = Vec::new();

    // Normalize: insert spaces around command letters, replace commas with spaces
    let mut normalized = String::with_capacity(d.len() * 2);
    for ch in d.chars() {
        if ch.is_ascii_alphabetic() {
            normalized.push(' ');
            normalized.push(ch);
            normalized.push(' ');
        } else if ch == ',' {
            normalized.push(' ');
        } else {
            normalized.push(ch);
        }
    }

    let tokens: Vec<&str> = normalized.split_whitespace().collect();
    let mut i = 0;
    let mut cur_x = 0.0_f32;
    let mut cur_y = 0.0_f32;

    macro_rules! parse_f32 {
        ($idx:expr) => {
            tokens.get($idx).and_then(|s| s.parse::<f32>().ok())?
        };
    }

    while i < tokens.len() {
        match tokens[i] {
            "M" => {
                let x = parse_f32!(i + 1);
                let y = parse_f32!(i + 2);
                cmds.push(PathCmd::MoveTo(x, y));
                cur_x = x;
                cur_y = y;
                i += 3;
            }
            "m" => {
                let dx = parse_f32!(i + 1);
                let dy = parse_f32!(i + 2);
                cmds.push(PathCmd::MoveTo(cur_x + dx, cur_y + dy));
                cur_x += dx;
                cur_y += dy;
                i += 3;
            }
            "L" => {
                let x = parse_f32!(i + 1);
                let y = parse_f32!(i + 2);
                cmds.push(PathCmd::LineTo(x, y));
                cur_x = x;
                cur_y = y;
                i += 3;
            }
            "l" => {
                let dx = parse_f32!(i + 1);
                let dy = parse_f32!(i + 2);
                cmds.push(PathCmd::LineTo(cur_x + dx, cur_y + dy));
                cur_x += dx;
                cur_y += dy;
                i += 3;
            }
            "H" => {
                let x = parse_f32!(i + 1);
                cmds.push(PathCmd::LineTo(x, cur_y));
                cur_x = x;
                i += 2;
            }
            "h" => {
                let dx = parse_f32!(i + 1);
                cmds.push(PathCmd::LineTo(cur_x + dx, cur_y));
                cur_x += dx;
                i += 2;
            }
            "V" => {
                let y = parse_f32!(i + 1);
                cmds.push(PathCmd::LineTo(cur_x, y));
                cur_y = y;
                i += 2;
            }
            "v" => {
                let dy = parse_f32!(i + 1);
                cmds.push(PathCmd::LineTo(cur_x, cur_y + dy));
                cur_y += dy;
                i += 2;
            }
            "C" => {
                let x1 = parse_f32!(i + 1);
                let y1 = parse_f32!(i + 2);
                let x2 = parse_f32!(i + 3);
                let y2 = parse_f32!(i + 4);
                let x = parse_f32!(i + 5);
                let y = parse_f32!(i + 6);
                cmds.push(PathCmd::CubicTo(x1, y1, x2, y2, x, y));
                cur_x = x;
                cur_y = y;
                i += 7;
            }
            "c" => {
                let dx1 = parse_f32!(i + 1);
                let dy1 = parse_f32!(i + 2);
                let dx2 = parse_f32!(i + 3);
                let dy2 = parse_f32!(i + 4);
                let dx = parse_f32!(i + 5);
                let dy = parse_f32!(i + 6);
                cmds.push(PathCmd::CubicTo(
                    cur_x + dx1,
                    cur_y + dy1,
                    cur_x + dx2,
                    cur_y + dy2,
                    cur_x + dx,
                    cur_y + dy,
                ));
                cur_x += dx;
                cur_y += dy;
                i += 7;
            }
            "Z" | "z" => {
                cmds.push(PathCmd::Close);
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    if cmds.is_empty() {
        None
    } else {
        Some(PathData(cmds))
    }
}

/// Adapter: [`PathData`] → tiny-skia `Path`.
pub(crate) fn to_tiny_path(p: &PathData) -> Option<tiny_skia::Path> {
    let mut pb = tiny_skia::PathBuilder::new();
    for cmd in &p.0 {
        match *cmd {
            PathCmd::MoveTo(x, y) => pb.move_to(x, y),
            PathCmd::LineTo(x, y) => pb.line_to(x, y),
            PathCmd::QuadTo(cx, cy, x, y) => pb.quad_to(cx, cy, x, y),
            PathCmd::CubicTo(x1, y1, x2, y2, x, y) => pb.cubic_to(x1, y1, x2, y2, x, y),
            PathCmd::Close => pb.close(),
        }
    }
    pb.finish()
}

/// Adapter: [`PathData`] → kurbo `BezPath` (vello).
pub(crate) fn to_kurbo_path(p: &PathData) -> vello::kurbo::BezPath {
    use vello::kurbo::{PathEl, Point};
    let mut path = vello::kurbo::BezPath::new();
    for cmd in &p.0 {
        match *cmd {
            PathCmd::MoveTo(x, y) => path.push(PathEl::MoveTo(Point::new(x as f64, y as f64))),
            PathCmd::LineTo(x, y) => path.push(PathEl::LineTo(Point::new(x as f64, y as f64))),
            PathCmd::QuadTo(cx, cy, x, y) => path.push(PathEl::QuadTo(
                Point::new(cx as f64, cy as f64),
                Point::new(x as f64, y as f64),
            )),
            PathCmd::CubicTo(x1, y1, x2, y2, x, y) => path.push(PathEl::CurveTo(
                Point::new(x1 as f64, y1 as f64),
                Point::new(x2 as f64, y2 as f64),
                Point::new(x as f64, y as f64),
            )),
            PathCmd::Close => path.push(PathEl::ClosePath),
        }
    }
    path
}
