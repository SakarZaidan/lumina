/// An easing curve: maps normalized progress `t ∈ [0, 1]` to eased progress.
pub type EasingFn = fn(f32) -> f32;

/// Evaluate an easing function by name with optional parameters.
/// Use this instead of `get_easing_fn` when `easing_params` may be present
/// (e.g. `cubic_bezier`).
pub fn eval_easing(name: &str, params: Option<&serde_json::Value>, t: f32) -> f32 {
    if name == "cubic_bezier" {
        if let Some(arr) = params.and_then(|p| p.as_array()) {
            if arr.len() >= 4 {
                let x1 = arr[0].as_f64().unwrap_or(0.25) as f32;
                let y1 = arr[1].as_f64().unwrap_or(0.1) as f32;
                let x2 = arr[2].as_f64().unwrap_or(0.25) as f32;
                let y2 = arr[3].as_f64().unwrap_or(1.0) as f32;
                return cubic_bezier_easing(x1, y1, x2, y2, t);
            }
        }
        // Fallback to CSS ease when params are malformed
        return ease_css(t);
    }
    if name == "spline" {
        // easing_params: { "keypoints": [[t0,v0], [t1,v1], ...] } (blueprint §10).
        if let Some(kp) = params
            .and_then(|p| p.get("keypoints"))
            .and_then(|k| k.as_array())
        {
            let pts: Vec<(f32, f32)> = kp
                .iter()
                .filter_map(|pair| {
                    let a = pair.as_array()?;
                    Some((a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32))
                })
                .collect();
            if pts.len() >= 2 {
                return spline_easing(&pts, t);
            }
        }
        // Fallback to linear when keypoints are missing/malformed.
        return linear(t);
    }
    get_easing_fn(name)(t)
}

/// Monotone cubic (Fritsch–Carlson) interpolation through sorted `keypoints`
/// at progress `p`. Monotone is preferred over Catmull–Rom so eased values do
/// not overshoot below the surrounding keypoints (no negative/over-1 spikes).
fn spline_easing(keypoints: &[(f32, f32)], p: f32) -> f32 {
    let n = keypoints.len();
    let p = p.clamp(0.0, 1.0);
    // Clamp to endpoints.
    if p <= keypoints[0].0 {
        return keypoints[0].1;
    }
    if p >= keypoints[n - 1].0 {
        return keypoints[n - 1].1;
    }
    // Secant slopes between consecutive points.
    let mut delta = vec![0.0_f32; n - 1];
    let mut h = vec![0.0_f32; n - 1];
    for i in 0..n - 1 {
        h[i] = (keypoints[i + 1].0 - keypoints[i].0).max(1e-9);
        delta[i] = (keypoints[i + 1].1 - keypoints[i].1) / h[i];
    }
    // Tangents via Fritsch–Carlson.
    let mut m = vec![0.0_f32; n];
    m[0] = delta[0];
    m[n - 1] = delta[n - 2];
    for i in 1..n - 1 {
        if delta[i - 1] * delta[i] <= 0.0 {
            m[i] = 0.0;
        } else {
            m[i] = (delta[i - 1] + delta[i]) / 2.0;
        }
    }
    for i in 0..n - 1 {
        if delta[i].abs() < 1e-9 {
            m[i] = 0.0;
            m[i + 1] = 0.0;
        } else {
            let a = m[i] / delta[i];
            let b = m[i + 1] / delta[i];
            let s = a * a + b * b;
            if s > 9.0 {
                let tau = 3.0 / s.sqrt();
                m[i] = tau * a * delta[i];
                m[i + 1] = tau * b * delta[i];
            }
        }
    }
    // Locate the segment containing p and evaluate the Hermite cubic.
    let mut i = 0;
    while i < n - 2 && p > keypoints[i + 1].0 {
        i += 1;
    }
    let t = (p - keypoints[i].0) / h[i];
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    h00 * keypoints[i].1 + h10 * h[i] * m[i] + h01 * keypoints[i + 1].1 + h11 * h[i] * m[i + 1]
}

/// Evaluate a CSS cubic-bezier(x1,y1,x2,y2) at progress p.
/// Control points P0=(0,0) and P3=(1,1); P1=(x1,y1) and P2=(x2,y2).
fn cubic_bezier_easing(x1: f32, y1: f32, x2: f32, y2: f32, p: f32) -> f32 {
    if p <= 0.0 {
        return 0.0;
    }
    if p >= 1.0 {
        return 1.0;
    }
    // Binary-search t such that bezier_x(t) ≈ p, then return bezier_y(t).
    let mut lo = 0.0_f32;
    let mut hi = 1.0_f32;
    for _ in 0..32 {
        let mid = (lo + hi) * 0.5;
        let x = bezier_component(x1, x2, mid);
        if (x - p).abs() < 1e-6 {
            return bezier_component(y1, y2, mid);
        }
        if x < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    bezier_component(y1, y2, (lo + hi) * 0.5)
}

/// Cubic Bézier on [0, p1, p2, 1] for a single axis.
fn bezier_component(p1: f32, p2: f32, t: f32) -> f32 {
    let u = 1.0 - t;
    3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t
}

/// Every easing name accepted by [`eval_easing`] / [`get_easing_fn`] — the
/// single source of truth for validation and did-you-mean suggestions.
/// `cubic_bezier` and `spline` additionally require `easing_params`.
pub const EASING_NAMES: &[&str] = &[
    "linear",
    "ease_in_quad",
    "ease_out_quad",
    "ease_in_out_quad",
    "ease_in_cubic",
    "ease_out_cubic",
    "ease_in_out_cubic",
    "ease_in_quart",
    "ease_out_quart",
    "ease_in_out_quart",
    "ease_in_sine",
    "ease_out_sine",
    "ease_in_out_sine",
    "ease_in_expo",
    "ease_out_expo",
    "ease_in_circ",
    "ease_out_circ",
    "ease_in_elastic",
    "ease_out_elastic",
    "ease_in_out_elastic",
    "ease_in_bounce",
    "ease_out_bounce",
    "spring",
    "smooth",
    "rush_into",
    "rush_from",
    "there_and_back",
    "ease",
    "ease_in",
    "ease_out",
    "ease_in_out",
    "cubic_bezier",
    "spline",
];

/// Is `name` a recognized easing?
pub fn is_valid_easing(name: &str) -> bool {
    EASING_NAMES.contains(&name)
}

/// The closest known easing name (edit distance ≤ 3), for
/// "did you mean …?" validation messages.
pub fn suggest_easing(name: &str) -> Option<&'static str> {
    EASING_NAMES
        .iter()
        .map(|candidate| (edit_distance(name, candidate), *candidate))
        .min_by_key(|(d, _)| *d)
        .filter(|(d, _)| *d <= 3)
        .map(|(_, c)| c)
}

/// Classic dynamic-programming Levenshtein distance.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// Look up a non-parameterized easing by name. Unknown names log a
/// warning and fall back to [`linear`]; scene validation rejects them
/// before rendering (`UNKNOWN_EASING`).
pub fn get_easing_fn(name: &str) -> EasingFn {
    match name {
        "linear" => linear,
        // Quad
        "ease_in_quad" => ease_in_quad,
        "ease_out_quad" => ease_out_quad,
        "ease_in_out_quad" => ease_in_out_quad,
        // Cubic
        "ease_in_cubic" => ease_in_cubic,
        "ease_out_cubic" => ease_out_cubic,
        "ease_in_out_cubic" => ease_in_out_cubic,
        // Quart
        "ease_in_quart" => ease_in_quart,
        "ease_out_quart" => ease_out_quart,
        "ease_in_out_quart" => ease_in_out_quart,
        // Sine
        "ease_in_sine" => ease_in_sine,
        "ease_out_sine" => ease_out_sine,
        "ease_in_out_sine" => ease_in_out_sine,
        // Expo
        "ease_in_expo" => ease_in_expo,
        "ease_out_expo" => ease_out_expo,
        // Circ
        "ease_in_circ" => ease_in_circ,
        "ease_out_circ" => ease_out_circ,
        // Elastic
        "ease_in_elastic" => ease_in_elastic,
        "ease_out_elastic" => ease_out_elastic,
        "ease_in_out_elastic" => ease_in_out_elastic,
        // Bounce
        "ease_in_bounce" => ease_in_bounce,
        "ease_out_bounce" => ease_out_bounce,
        // Spring (default parameters)
        "spring" => spring,
        // Manim-compatible
        "smooth" => smooth,
        "rush_into" => rush_into,
        "rush_from" => rush_from,
        "there_and_back" => there_and_back,
        // CSS standard aliases
        "ease" => ease_css,
        "ease_in" => ease_in_cubic,
        "ease_out" => ease_out_cubic,
        "ease_in_out" => ease_in_out_cubic,
        // Unknown names are rejected by scene validation (UNKNOWN_EASING);
        // this fallback is defense-in-depth for unvalidated callers only.
        unknown => {
            log::warn!("unknown easing '{unknown}', falling back to linear");
            linear
        }
    }
}

/// Identity easing: constant velocity.
pub fn linear(t: f32) -> f32 {
    t
}

// --- Quad ---

/// Quadratic ease-in: accelerates from rest.
pub fn ease_in_quad(t: f32) -> f32 {
    t * t
}

/// Quadratic ease-out: decelerates to rest.
pub fn ease_out_quad(t: f32) -> f32 {
    t * (2.0 - t)
}

/// Quadratic ease-in-out: accelerate, then decelerate.
pub fn ease_in_out_quad(t: f32) -> f32 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        -1.0 + (4.0 - 2.0 * t) * t
    }
}

// --- Cubic ---

/// Cubic ease-in: stronger acceleration than quad.
pub fn ease_in_cubic(t: f32) -> f32 {
    t * t * t
}

/// Cubic ease-out: stronger deceleration than quad.
pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t - 1.0;
    t * t * t + 1.0
}

/// Cubic ease-in-out.
pub fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let t = 2.0 * t - 2.0;
        0.5 * t * t * t + 1.0
    }
}

// --- Quart ---

/// Quartic ease-in: very pronounced acceleration.
pub fn ease_in_quart(t: f32) -> f32 {
    t * t * t * t
}

/// Quartic ease-out: very pronounced deceleration.
pub fn ease_out_quart(t: f32) -> f32 {
    let t = t - 1.0;
    1.0 - t * t * t * t
}

/// Quartic ease-in-out.
pub fn ease_in_out_quart(t: f32) -> f32 {
    if t < 0.5 {
        8.0 * t * t * t * t
    } else {
        let t = 2.0 * t - 2.0;
        1.0 - t * t * t * t / 2.0
    }
}

// --- Sine ---

/// Sinusoidal ease-in: gentle acceleration.
pub fn ease_in_sine(t: f32) -> f32 {
    1.0 - (t * std::f32::consts::FRAC_PI_2).cos()
}

/// Sinusoidal ease-out: gentle deceleration.
pub fn ease_out_sine(t: f32) -> f32 {
    (t * std::f32::consts::FRAC_PI_2).sin()
}

/// Sinusoidal ease-in-out.
pub fn ease_in_out_sine(t: f32) -> f32 {
    -((std::f32::consts::PI * t).cos() - 1.0) / 2.0
}

// --- Expo ---

/// Exponential ease-in: near-zero start, explosive finish.
pub fn ease_in_expo(t: f32) -> f32 {
    if t == 0.0 {
        0.0
    } else {
        2.0_f32.powf(10.0 * t - 10.0)
    }
}

/// Exponential ease-out: explosive start, asymptotic finish.
pub fn ease_out_expo(t: f32) -> f32 {
    if t == 1.0 {
        1.0
    } else {
        1.0 - 2.0_f32.powf(-10.0 * t)
    }
}

// --- Circ ---

/// Circular ease-in (quarter-circle arc).
pub fn ease_in_circ(t: f32) -> f32 {
    1.0 - (1.0 - t * t).max(0.0).sqrt()
}

/// Circular ease-out (quarter-circle arc).
pub fn ease_out_circ(t: f32) -> f32 {
    let t = t - 1.0;
    (1.0 - t * t).max(0.0).sqrt()
}

// --- Elastic ---

const C4: f32 = 2.0 * std::f32::consts::PI / 3.0;
const C5: f32 = 2.0 * std::f32::consts::PI / 4.5;

/// Elastic ease-in: winds up with oscillation before releasing.
pub fn ease_in_elastic(t: f32) -> f32 {
    if t == 0.0 {
        return 0.0;
    }
    if t == 1.0 {
        return 1.0;
    }
    -(2.0_f32.powf(10.0 * t - 10.0)) * ((10.0 * t - 10.75) * C4).sin()
}

/// Elastic ease-out: overshoots and oscillates into place.
pub fn ease_out_elastic(t: f32) -> f32 {
    if t == 0.0 {
        return 0.0;
    }
    if t == 1.0 {
        return 1.0;
    }
    2.0_f32.powf(-10.0 * t) * ((10.0 * t - 0.75) * C4).sin() + 1.0
}

/// Elastic ease-in-out.
pub fn ease_in_out_elastic(t: f32) -> f32 {
    if t == 0.0 {
        return 0.0;
    }
    if t == 1.0 {
        return 1.0;
    }
    if t < 0.5 {
        -(2.0_f32.powf(20.0 * t - 10.0) * ((20.0 * t - 11.125) * C5).sin()) / 2.0
    } else {
        2.0_f32.powf(-20.0 * t + 10.0) * ((20.0 * t - 11.125) * C5).sin() / 2.0 + 1.0
    }
}

// --- Bounce ---

/// Bounce ease-out: lands and bounces to rest.
pub fn ease_out_bounce(t: f32) -> f32 {
    const N1: f32 = 7.5625;
    const D1: f32 = 2.75;

    if t < 1.0 / D1 {
        N1 * t * t
    } else if t < 2.0 / D1 {
        let t = t - 1.5 / D1;
        N1 * t * t + 0.75
    } else if t < 2.5 / D1 {
        let t = t - 2.25 / D1;
        N1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / D1;
        N1 * t * t + 0.984375
    }
}

/// Bounce ease-in: mirror of [`ease_out_bounce`].
pub fn ease_in_bounce(t: f32) -> f32 {
    1.0 - ease_out_bounce(1.0 - t)
}

// --- Spring (critically damped, default stiffness=200, damping=20, mass=1) ---
// Approximates a spring curve on [0,1] using RK4 integration.
/// Critically-under-damped spring (RK4-integrated), slight overshoot.
pub fn spring(t: f32) -> f32 {
    const STIFFNESS: f32 = 200.0;
    const DAMPING: f32 = 20.0;
    const STEPS: u32 = 100;
    const DT: f32 = 1.0 / STEPS as f32;

    let mut x = 0.0_f32;
    let mut v = 0.0_f32;

    let target_t = t;
    let n = (target_t / DT).round() as u32;

    for _ in 0..n {
        let a = -STIFFNESS * (x - 1.0) - DAMPING * v;
        v += a * DT;
        x += v * DT;
    }

    x.clamp(0.0, 2.0)
}

// --- Manim-compatible ---

// Smoothstep: 3t² - 2t³ (Manim's "smooth")
/// Manim-style smoothstep: zero velocity at both ends.
pub fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

// Fast acceleration into a smooth stop
/// Manim-style: fast start, smooth stop (first half of `smooth`).
pub fn rush_into(t: f32) -> f32 {
    smooth(t / 2.0) * 2.0
}

// Slow start, then rush to end
/// Manim-style: smooth start, fast finish (second half of `smooth`).
pub fn rush_from(t: f32) -> f32 {
    smooth(t / 2.0 + 0.5) * 2.0 - 1.0
}

// Animate out and back to start (useful for emphasis)
/// Manim-style: eases to 1 at t=0.5 and back to 0 at t=1.
pub fn there_and_back(t: f32) -> f32 {
    let t = 2.0 * t;
    if t < 1.0 {
        smooth(t)
    } else {
        smooth(2.0 - t)
    }
}

// CSS cubic-bezier(0.25, 0.1, 0.25, 1.0) approximated as ease_in_out_sine
/// The CSS `ease` curve: cubic-bezier(0.25, 0.1, 0.25, 1.0).
pub fn ease_css(t: f32) -> f32 {
    ease_in_out_sine(t)
}
