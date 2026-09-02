//! Sampling for `Plot` objects: one implementation shared by both backends.
//!
//! Deciding *where* to sample a function is a rendering decision, not a
//! drawing one, so it belongs here rather than being written twice (TD-02,
//! `ENGINEERING_PRINCIPLES` #4). The backends receive polylines and emit them.
//!
//! Three things this fixes over the per-backend loops it replaces.
//!
//! **The expression was reparsed on every sample.** At the default 200 samples
//! over a minute of 60 fps output that is 720 000 parses of one string that
//! never changes. It is parsed once per plot now.
//!
//! **Sampling was uniform.** A fixed grid facets steep regions and wastes
//! points on flat ones. Subdivision driven by chord deviation puts samples
//! where the curve actually bends.
//!
//! **The domain was `f32`.** Plot ranges are author-supplied and can be wide;
//! f32 gives about seven significant digits, so a domain like `[0, 1e6]` lost
//! resolution before the renderer ever saw it. Sampling is `f64` throughout and
//! narrows only at the screen-space boundary.

use evalexpr::{
    build_operator_tree, ContextWithMutableVariables, HashMapContext, Node, Value as EvalValue,
};

/// A connected run of points in math space. A plot is a list of these:
/// discontinuities and out-of-range excursions split the curve rather than
/// drawing a false vertical line across the asymptote.
pub type Segment = Vec<(f64, f64)>;

/// Bare function names rewritten into evalexpr's `math::` namespace.
///
/// Longest first so `asin` is matched before `sin` — the previous
/// implementation used `str::replace("sin(", "math::sin(")`, which turned
/// `asin(x)` into `amath::sin(x)`.
const MATH_FNS: &[&str] = &[
    "asinh", "acosh", "atanh", "asin", "acos", "atan2", "atan", "sinh", "cosh", "tanh", "sqrt",
    "cbrt", "ln", "log2", "log10", "log", "exp2", "exp", "abs", "floor", "ceil", "round", "sin",
    "cos", "tan", "hypot", "pow",
];

/// Rewrite bare math calls into evalexpr's namespace.
///
/// Scans for identifier boundaries instead of substring-replacing, so a name
/// that merely *contains* another is left alone, and a call already written as
/// `math::sin` is not prefixed twice. The old version bailed out of
/// normalisation entirely if the expression contained `math::` anywhere, so
/// mixing styles in one expression silently broke the bare half.
pub fn normalize_math_calls(expr: &str) -> String {
    let bytes = expr.as_bytes();
    let mut out = String::with_capacity(expr.len() + 16);
    let mut i = 0;

    while i < bytes.len() {
        let c = bytes[i] as char;
        if !c.is_ascii_alphabetic() && c != '_' {
            out.push(c);
            i += 1;
            continue;
        }

        // Read one identifier.
        let start = i;
        while i < bytes.len() {
            let c = bytes[i] as char;
            if c.is_ascii_alphanumeric() || c == '_' {
                i += 1;
            } else {
                break;
            }
        }
        let ident = &expr[start..i];

        // Already namespaced? Copy it and the `::` through untouched.
        if expr[i..].starts_with("::") {
            out.push_str(ident);
            continue;
        }
        // A call is an identifier immediately followed by `(`.
        let is_call = expr[i..].starts_with('(');
        let already_prefixed = start >= 6 && expr[..start].ends_with("math::");

        if is_call && !already_prefixed && MATH_FNS.contains(&ident) {
            out.push_str("math::");
        }
        out.push_str(ident);
    }
    out
}

/// A parsed plot function, ready to evaluate many times.
pub(crate) struct PlotFn {
    tree: Node,
    ctx: HashMapContext,
}

impl PlotFn {
    /// Parse `expr` once. Returns `None` if it is not a valid expression.
    pub(crate) fn parse(expr: &str) -> Option<Self> {
        let tree = build_operator_tree(&normalize_math_calls(expr)).ok()?;
        Some(Self {
            tree,
            ctx: HashMapContext::new(),
        })
    }

    /// Evaluate at `x`, or `None` where the function is undefined.
    fn eval(&mut self, x: f64) -> Option<f64> {
        self.ctx.set_value("x".into(), EvalValue::Float(x)).ok()?;
        let y = self.tree.eval_number_with_context(&self.ctx).ok()?;
        y.is_finite().then_some(y)
    }
}

/// How far the plot may stray outside the visible y-range before the curve is
/// treated as having left the frame. One full range on each side keeps a curve
/// that dips just off-screen connected, while still cutting at a pole.
const OFF_SCREEN_RANGES: f64 = 1.0;

/// Adaptive subdivision limits.
///
/// The recursion depth bounds the worst case; the sample budget bounds the
/// total. Both matter because `sample_count` is author-supplied and a plot is
/// re-sampled on every frame.
const MAX_DEPTH: u32 = 12;
/// Initial uniform points before refinement, as a fraction of the budget.
const SEED_FRACTION: usize = 16;
/// Never seed with fewer than this, however small the budget.
const MIN_SEED: usize = 16;

/// Sample `expr` over `[x_min, x_max]`, returning connected polylines.
///
/// `y_min`/`y_max` describe the visible range: they set both the flatness
/// tolerance and the point at which the curve is considered to have left the
/// frame. `budget` caps total evaluations.
pub fn sample(
    expr: &str,
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
    budget: usize,
) -> Vec<Segment> {
    let Some(mut f) = PlotFn::parse(expr) else {
        return Vec::new();
    };
    if !(x_min.is_finite() && x_max.is_finite()) || x_max <= x_min || budget == 0 {
        return Vec::new();
    }

    let y_span = (y_max - y_min).abs().max(f64::EPSILON);
    let in_frame =
        |y: f64| y >= y_min - y_span * OFF_SCREEN_RANGES && y <= y_max + y_span * OFF_SCREEN_RANGES;
    // Deviation from the chord, in math units, below which a span is flat
    // enough to draw as one line. Scaled to the visible range so the criterion
    // means the same thing whatever the axes describe.
    let tolerance = y_span * 1e-3;

    let seed = (budget / SEED_FRACTION).max(MIN_SEED).min(budget);
    let mut used = 0usize;
    let mut segments: Vec<Segment> = Vec::new();
    let mut current: Segment = Vec::new();

    let mut prev: Option<(f64, f64)> = None;
    for i in 0..=seed {
        let x = x_min + (x_max - x_min) * (i as f64 / seed as f64);
        let y = f.eval(x).filter(|y| in_frame(*y));
        used += 1;

        match (prev, y) {
            (Some(p), Some(y)) => {
                refine(
                    &mut f,
                    p,
                    (x, y),
                    tolerance,
                    &in_frame,
                    0,
                    &mut used,
                    budget,
                    &mut current,
                );
                current.push((x, y));
                prev = Some((x, y));
            }
            (None, Some(y)) => {
                current.push((x, y));
                prev = Some((x, y));
            }
            (_, None) => {
                // Undefined or off-frame: close the run. Walking a little way
                // toward the break keeps the curve from stopping short of a
                // pole it is racing toward.
                if let Some(p) = prev {
                    if let Some(edge) = approach_break(&mut f, p.0, x, &in_frame, &mut used, budget)
                    {
                        current.push(edge);
                    }
                }
                if current.len() > 1 {
                    segments.push(std::mem::take(&mut current));
                } else {
                    current.clear();
                }
                prev = None;
            }
        }
        if used >= budget {
            break;
        }
    }
    if current.len() > 1 {
        segments.push(current);
    }
    segments
}

/// Subdivide `(a, b)` until the midpoint lies close enough to the chord.
#[allow(clippy::too_many_arguments)]
fn refine(
    f: &mut PlotFn,
    a: (f64, f64),
    b: (f64, f64),
    tolerance: f64,
    in_frame: &impl Fn(f64) -> bool,
    depth: u32,
    used: &mut usize,
    budget: usize,
    out: &mut Segment,
) {
    if depth >= MAX_DEPTH || *used >= budget {
        return;
    }
    let mx = 0.5 * (a.0 + b.0);
    let Some(my) = f.eval(mx).filter(|y| in_frame(*y)) else {
        return;
    };
    *used += 1;

    let chord = 0.5 * (a.1 + b.1);
    if (my - chord).abs() <= tolerance {
        return; // flat enough: the straight line between a and b will do
    }
    refine(
        f,
        a,
        (mx, my),
        tolerance,
        in_frame,
        depth + 1,
        used,
        budget,
        out,
    );
    out.push((mx, my));
    refine(
        f,
        (mx, my),
        b,
        tolerance,
        in_frame,
        depth + 1,
        used,
        budget,
        out,
    );
}

/// Binary-search from a defined `x_ok` toward an undefined `x_bad` for the last
/// point still on screen, so a curve reaches its asymptote instead of stopping
/// at whatever the sampling grid happened to land on.
fn approach_break(
    f: &mut PlotFn,
    x_ok: f64,
    x_bad: f64,
    in_frame: &impl Fn(f64) -> bool,
    used: &mut usize,
    budget: usize,
) -> Option<(f64, f64)> {
    let (mut lo, mut hi) = (x_ok, x_bad);
    let mut best: Option<(f64, f64)> = None;
    for _ in 0..24 {
        if *used >= budget {
            break;
        }
        let mid = 0.5 * (lo + hi);
        *used += 1;
        match f.eval(mid).filter(|y| in_frame(*y)) {
            Some(y) => {
                best = Some((mid, y));
                lo = mid;
            }
            None => hi = mid,
        }
    }
    best
}
