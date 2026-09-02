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
pub struct PathData(pub(crate) Vec<PathCmd>);

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
/// What went wrong while parsing SVG path data, and where.
///
/// The previous parser returned `None` for any problem, so a single bad
/// coordinate silently dropped an entire shape with nothing to act on — the
/// worst failure mode for a declarative format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathError {
    /// Byte offset into the path data where the problem was found.
    pub offset: usize,
    /// The token that could not be used.
    pub token: String,
    /// What was expected instead.
    pub expected: String,
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "at offset {}: expected {}, found {:?}",
            self.offset, self.expected, self.token
        )
    }
}

impl std::error::Error for PathError {}

/// One lexed item of path data: a command letter or a number.
#[derive(Debug, Clone, Copy)]
enum Tok<'a> {
    Cmd(char, usize),
    Num(f32, &'a str, usize),
}

/// Split path data into commands and numbers.
///
/// SVG's grammar allows numbers to run together without separators —
/// `M0 0-1-1` and `1.5.5` (two numbers) are both legal — so this cannot be a
/// `split_whitespace`. A number ends when a sign appears that is not an
/// exponent's, or when a second decimal point appears.
fn lex(d: &str) -> Result<Vec<Tok<'_>>, PathError> {
    let bytes = d.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_whitespace() || c == ',' {
            i += 1;
        } else if c.is_ascii_alphabetic() {
            out.push(Tok::Cmd(c, i));
            i += 1;
        } else {
            let start = i;
            let mut seen_dot = false;
            let mut seen_digit = false;
            if matches!(bytes[i] as char, '+' | '-') {
                i += 1;
            }
            while i < bytes.len() {
                match bytes[i] as char {
                    '0'..='9' => {
                        seen_digit = true;
                        i += 1;
                    }
                    '.' if !seen_dot => {
                        seen_dot = true;
                        i += 1;
                    }
                    'e' | 'E' if seen_digit => {
                        i += 1;
                        if i < bytes.len() && matches!(bytes[i] as char, '+' | '-') {
                            i += 1;
                        }
                    }
                    _ => break,
                }
            }
            let text = &d[start..i];
            let value = text.parse::<f32>().map_err(|_| PathError {
                offset: start,
                token: text.to_string(),
                expected: "a number".to_string(),
            })?;
            if !value.is_finite() {
                return Err(PathError {
                    offset: start,
                    token: text.to_string(),
                    expected: "a finite number".to_string(),
                });
            }
            out.push(Tok::Num(value, text, start));
        }
    }
    Ok(out)
}

/// Parse SVG path data into backend-neutral commands.
///
/// Supports the full command set — `M/m L/l H/h V/v C/c S/s Q/q T/t A/a Z/z` —
/// including **repeated coordinate sets**, where `L 1 1 2 2` draws two lines
/// and a repeat after `M` becomes an implicit `L`, as the specification
/// requires. Real files from vector editors rely on both constantly; the
/// previous parser handled neither, so curves and repeats were silently lost.
///
/// Elliptical arcs are converted to cubic Béziers, since a Bézier is what both
/// backends ultimately draw.
///
/// # Errors
///
/// Returns [`PathError`] naming the offending token and its offset, rather
/// than discarding the whole path.
pub fn parse_svg_path_detailed(d: &str) -> Result<PathData, PathError> {
    let toks = lex(d)?;
    let mut cmds: Vec<PathCmd> = Vec::new();
    let (mut cx, mut cy) = (0.0f32, 0.0f32);
    // Start of the current subpath, for `Z`.
    let (mut sx, mut sy) = (0.0f32, 0.0f32);
    // Previous curve's second control point, reflected by `S` and `T`.
    let mut last_cubic_ctrl: Option<(f32, f32)> = None;
    let mut last_quad_ctrl: Option<(f32, f32)> = None;

    let mut i = 0;
    let mut cmd: Option<char> = None;

    while i < toks.len() {
        // A command letter, or a repeat of the previous command's operands.
        match toks[i] {
            Tok::Cmd(c, _) => {
                cmd = Some(c);
                i += 1;
            }
            Tok::Num(_, text, offset) if cmd.is_none() => {
                return Err(PathError {
                    offset,
                    token: text.to_string(),
                    expected: "a command letter before any coordinates".to_string(),
                });
            }
            Tok::Num(..) => {}
        }

        let Some(c) = cmd else { break };

        // `Z` takes no operands; everything else needs some.
        if matches!(c, 'Z' | 'z') {
            cmds.push(PathCmd::Close);
            cx = sx;
            cy = sy;
            last_cubic_ctrl = None;
            last_quad_ctrl = None;
            continue;
        }

        let need = match c {
            'M' | 'm' | 'L' | 'l' | 'T' | 't' => 2,
            'H' | 'h' | 'V' | 'v' => 1,
            'S' | 's' | 'Q' | 'q' => 4,
            'C' | 'c' => 6,
            'A' | 'a' => 7,
            other => {
                return Err(PathError {
                    offset: match toks[i.saturating_sub(1)] {
                        Tok::Cmd(_, o) | Tok::Num(_, _, o) => o,
                    },
                    token: other.to_string(),
                    expected: "one of M L H V C S Q T A Z".to_string(),
                })
            }
        };

        let mut arg = [0.0f32; 7];
        for (n, slot) in arg.iter_mut().enumerate().take(need) {
            match toks.get(i + n) {
                Some(Tok::Num(v, ..)) => *slot = *v,
                _ => {
                    return Err(PathError {
                        offset: d.len(),
                        token: format!("{c}"),
                        expected: format!("{need} numbers after '{c}'"),
                    })
                }
            }
        }
        i += need;

        let rel = c.is_ascii_lowercase();
        let (ox, oy) = if rel { (cx, cy) } else { (0.0, 0.0) };

        match c.to_ascii_uppercase() {
            'M' => {
                let (x, y) = (ox + arg[0], oy + arg[1]);
                cmds.push(PathCmd::MoveTo(x, y));
                (cx, cy) = (x, y);
                (sx, sy) = (x, y);
                // Per the specification, further coordinate pairs after a
                // moveto are treated as implicit linetos.
                cmd = Some(if rel { 'l' } else { 'L' });
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
            }
            'L' => {
                let (x, y) = (ox + arg[0], oy + arg[1]);
                cmds.push(PathCmd::LineTo(x, y));
                (cx, cy) = (x, y);
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
            }
            'H' => {
                let x = ox + arg[0];
                cmds.push(PathCmd::LineTo(x, cy));
                cx = x;
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
            }
            'V' => {
                let y = oy + arg[0];
                cmds.push(PathCmd::LineTo(cx, y));
                cy = y;
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
            }
            'C' => {
                let (x1, y1) = (ox + arg[0], oy + arg[1]);
                let (x2, y2) = (ox + arg[2], oy + arg[3]);
                let (x, y) = (ox + arg[4], oy + arg[5]);
                cmds.push(PathCmd::CubicTo(x1, y1, x2, y2, x, y));
                (cx, cy) = (x, y);
                last_cubic_ctrl = Some((x2, y2));
                last_quad_ctrl = None;
            }
            'S' => {
                // Smooth cubic: the first control point mirrors the previous
                // curve's second one about the current point.
                let (x1, y1) = match last_cubic_ctrl {
                    Some((px, py)) => (2.0 * cx - px, 2.0 * cy - py),
                    None => (cx, cy),
                };
                let (x2, y2) = (ox + arg[0], oy + arg[1]);
                let (x, y) = (ox + arg[2], oy + arg[3]);
                cmds.push(PathCmd::CubicTo(x1, y1, x2, y2, x, y));
                (cx, cy) = (x, y);
                last_cubic_ctrl = Some((x2, y2));
                last_quad_ctrl = None;
            }
            'Q' => {
                let (x1, y1) = (ox + arg[0], oy + arg[1]);
                let (x, y) = (ox + arg[2], oy + arg[3]);
                cmds.push(PathCmd::QuadTo(x1, y1, x, y));
                (cx, cy) = (x, y);
                last_quad_ctrl = Some((x1, y1));
                last_cubic_ctrl = None;
            }
            'T' => {
                // Smooth quadratic: the control point mirrors the previous
                // quadratic's about the current point.
                let (x1, y1) = match last_quad_ctrl {
                    Some((px, py)) => (2.0 * cx - px, 2.0 * cy - py),
                    None => (cx, cy),
                };
                let (x, y) = (ox + arg[0], oy + arg[1]);
                cmds.push(PathCmd::QuadTo(x1, y1, x, y));
                (cx, cy) = (x, y);
                last_quad_ctrl = Some((x1, y1));
                last_cubic_ctrl = None;
            }
            'A' => {
                let (x, y) = (ox + arg[5], oy + arg[6]);
                arc_to_cubics(
                    &mut cmds,
                    (cx, cy),
                    (arg[0], arg[1]),
                    arg[2],
                    arg[3] != 0.0,
                    arg[4] != 0.0,
                    (x, y),
                );
                (cx, cy) = (x, y);
                last_cubic_ctrl = None;
                last_quad_ctrl = None;
            }
            _ => unreachable!("command set checked above"),
        }
    }

    if cmds.is_empty() {
        return Err(PathError {
            offset: 0,
            token: d.chars().take(16).collect(),
            expected: "at least one path command".to_string(),
        });
    }
    Ok(PathData(cmds))
}

/// Parse SVG path data, discarding the reason on failure.
///
/// Prefer [`parse_svg_path_detailed`] where the error can be surfaced.
pub fn parse_svg_path(d: &str) -> Option<PathData> {
    parse_svg_path_detailed(d).ok()
}

/// Append an elliptical arc as cubic Béziers.
///
/// Implements the endpoint-to-centre conversion from the SVG 1.1
/// specification, appendix F.6.5, then emits one cubic per segment of at most
/// 90 degrees — the angle beyond which a cubic approximation of a circular arc
/// visibly deviates.
///
/// Degenerate inputs are handled the way the specification requires: a zero
/// radius, or identical endpoints, becomes a straight line rather than an
/// error, because that is what a conforming renderer draws.
#[allow(clippy::too_many_arguments)]
fn arc_to_cubics(
    out: &mut Vec<PathCmd>,
    from: (f32, f32),
    radii: (f32, f32),
    x_rotation_deg: f32,
    large_arc: bool,
    sweep: bool,
    to: (f32, f32),
) {
    let (x1, y1) = (f64::from(from.0), f64::from(from.1));
    let (x2, y2) = (f64::from(to.0), f64::from(to.1));
    let (mut rx, mut ry) = (f64::from(radii.0).abs(), f64::from(radii.1).abs());

    if rx < 1e-9 || ry < 1e-9 || ((x1 - x2).abs() < 1e-9 && (y1 - y2).abs() < 1e-9) {
        out.push(PathCmd::LineTo(to.0, to.1));
        return;
    }

    let phi = f64::from(x_rotation_deg).to_radians();
    let (cos_phi, sin_phi) = (phi.cos(), phi.sin());

    // Step 1: endpoint to the ellipse's own coordinate frame.
    let dx2 = (x1 - x2) / 2.0;
    let dy2 = (y1 - y2) / 2.0;
    let x1p = cos_phi * dx2 + sin_phi * dy2;
    let y1p = -sin_phi * dx2 + cos_phi * dy2;

    // Step 2: scale the radii up if they are too small to span the endpoints.
    let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
    if lambda > 1.0 {
        let scale = lambda.sqrt();
        rx *= scale;
        ry *= scale;
    }

    let sign = if large_arc == sweep { -1.0 } else { 1.0 };
    let num = (rx * rx * ry * ry) - (rx * rx * y1p * y1p) - (ry * ry * x1p * x1p);
    let den = (rx * rx * y1p * y1p) + (ry * ry * x1p * x1p);
    let coef = sign * (num.max(0.0) / den).sqrt();
    let cxp = coef * (rx * y1p) / ry;
    let cyp = coef * -(ry * x1p) / rx;

    // Step 3: back to user space.
    let cx = cos_phi * cxp - sin_phi * cyp + (x1 + x2) / 2.0;
    let cy = sin_phi * cxp + cos_phi * cyp + (y1 + y2) / 2.0;

    // Step 4: start angle and sweep.
    let angle = |ux: f64, uy: f64, vx: f64, vy: f64| -> f64 {
        let dot = ux * vx + uy * vy;
        let len = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
        let mut a = (dot / len).clamp(-1.0, 1.0).acos();
        if ux * vy - uy * vx < 0.0 {
            a = -a;
        }
        a
    };
    let theta1 = angle(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry);
    let mut delta = angle(
        (x1p - cxp) / rx,
        (y1p - cyp) / ry,
        (-x1p - cxp) / rx,
        (-y1p - cyp) / ry,
    );
    if !sweep && delta > 0.0 {
        delta -= std::f64::consts::TAU;
    } else if sweep && delta < 0.0 {
        delta += std::f64::consts::TAU;
    }

    // Step 5: one cubic per <=90 degree segment.
    let segments = (delta.abs() / std::f64::consts::FRAC_PI_2).ceil().max(1.0) as usize;
    let step = delta / segments as f64;
    // Standard control-point distance for approximating a circular arc with a
    // cubic Bézier.
    let alpha = (4.0 / 3.0) * (step / 4.0).tan();

    let mut theta = theta1;
    for _ in 0..segments {
        let (cos1, sin1) = (theta.cos(), theta.sin());
        let theta2 = theta + step;
        let (cos2, sin2) = (theta2.cos(), theta2.sin());

        let point = |ct: f64, st: f64| -> (f64, f64) {
            (
                cx + rx * ct * cos_phi - ry * st * sin_phi,
                cy + rx * ct * sin_phi + ry * st * cos_phi,
            )
        };
        let deriv = |ct: f64, st: f64| -> (f64, f64) {
            (
                -rx * st * cos_phi - ry * ct * sin_phi,
                -rx * st * sin_phi + ry * ct * cos_phi,
            )
        };

        let (px1, py1) = point(cos1, sin1);
        let (dx1, dy1) = deriv(cos1, sin1);
        let (px2, py2) = point(cos2, sin2);
        let (dx2b, dy2b) = deriv(cos2, sin2);

        out.push(PathCmd::CubicTo(
            (px1 + alpha * dx1) as f32,
            (py1 + alpha * dy1) as f32,
            (px2 - alpha * dx2b) as f32,
            (py2 - alpha * dy2b) as f32,
            px2 as f32,
            py2 as f32,
        ));
        theta = theta2;
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
