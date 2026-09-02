use crate::easing::eval_easing;
use serde_json::Value;

/// Interpolate between two JSON values at normalized progress `t` in `[0, 1]`.
///
/// - Numbers: linear lerp after easing.
/// - Arrays: element-wise interpolation; shorter array is padded by repeating
///   its last element, enabling path morphing between point lists of different
///   lengths.
/// - Hex color strings (`#RGB`, `#RGBA`, `#RRGGBB`, `#RRGGBBAA`): interpolated
///   in `OKLab`, with alpha blended linearly.
/// - All other types: snap to `v2`.
pub fn interpolate_value(
    v1: &Value,
    v2: &Value,
    t: f32,
    easing_name: &str,
    easing_params: Option<&Value>,
) -> Value {
    let t = eval_easing(easing_name, easing_params, t);

    match (v1, v2) {
        (Value::Number(n1), Value::Number(n2)) => {
            let f1 = n1.as_f64().unwrap_or(0.0) as f32;
            let f2 = n2.as_f64().unwrap_or(0.0) as f32;
            let lerped = f1 + (f2 - f1) * t;
            // `Value::from` maps NaN and infinity to `Value::Null`, and a null
            // property does not fail loudly — it disappears from the state map
            // and the renderer falls back to its own default, so the animation
            // is wrong with nothing to say so. Values large enough to overflow
            // the subtraction are rejected by validation; this is the second
            // line of defence, degrading to the nearest endpoint rather than
            // to nothing.
            if lerped.is_finite() {
                Value::from(lerped)
            } else if t < 0.5 {
                Value::from(if f1.is_finite() { f1 } else { 0.0 })
            } else {
                Value::from(if f2.is_finite() { f2 } else { 0.0 })
            }
        }
        (Value::Array(a1), Value::Array(a2)) => {
            // Pad shorter array with its last element so paths of different
            // vertex counts can morph into each other.
            let len = a1.len().max(a2.len());
            let null = Value::Null;
            let mut result = Vec::with_capacity(len);
            for i in 0..len {
                let e1 = a1.get(i).or_else(|| a1.last()).unwrap_or(&null);
                let e2 = a2.get(i).or_else(|| a2.last()).unwrap_or(&null);
                // t is already eased; inner calls use "linear" (identity).
                result.push(interpolate_value(e1, e2, t, "linear", None));
            }
            Value::Array(result)
        }
        (Value::String(s1), Value::String(s2)) => {
            // Hex colours blend in OKLab, which is perceptually uniform, so a
            // fade passes through the colours a viewer expects rather than
            // through a muddy or drifting midpoint.
            if let (Some(c1), Some(c2)) = (parse_hex_color(s1), parse_hex_color(s2)) {
                let (linear, alpha) = mix_linear(&c1, &c2, t);
                // Widen to eight digits only if either side asked for it, so
                // `#FF0000` stays `#RRGGBB` through a fade.
                let with_alpha = c1.has_alpha || c2.has_alpha;
                return Value::String(to_hex(linear, alpha, with_alpha));
            }
            v2.clone()
        }
        _ => v2.clone(),
    }
}

/// Blend two colours perceptually, returning linear-light RGB plus alpha.
///
/// Colour blends in `OKLab`; alpha blends linearly, because alpha is a coverage
/// fraction rather than a perceptual quantity and running it through a
/// perceptual space would be a category error.
fn mix_linear(a: &Rgba, b: &Rgba, t: f32) -> ([f32; 3], f32) {
    let lab1 = linear_to_oklab(a.linear);
    let lab2 = linear_to_oklab(b.linear);
    let lab = [
        lab1[0] + (lab2[0] - lab1[0]) * t,
        lab1[1] + (lab2[1] - lab1[1]) * t,
        lab1[2] + (lab2[2] - lab1[2]) * t,
    ];
    (oklab_to_linear(lab), a.alpha + (b.alpha - a.alpha) * t)
}

/// Blend two straight-alpha RGBA8 colours the way a keyframe fade does.
///
/// Exposed so the renderer can build gradient stops through the **same**
/// implementation the timeline uses. Gradients previously interpolated in
/// sRGB while keyframes interpolated perceptually, so the same two colours
/// produced two different midpoints depending on whether they met in a
/// gradient or in an animation. Sharing the function makes them agree by
/// construction rather than by coincidence.
#[must_use]
pub fn mix_rgba8(a: [u8; 4], b: [u8; 4], t: f32) -> [u8; 4] {
    let to_rgba = |c: [u8; 4]| Rgba {
        linear: [
            srgb_to_linear(f32::from(c[0]) / 255.0),
            srgb_to_linear(f32::from(c[1]) / 255.0),
            srgb_to_linear(f32::from(c[2]) / 255.0),
        ],
        alpha: f32::from(c[3]) / 255.0,
        has_alpha: true,
    };
    let (linear, alpha) = mix_linear(&to_rgba(a), &to_rgba(b), t.clamp(0.0, 1.0));
    let ch = |v: f32| (linear_to_srgb(v.clamp(0.0, 1.0)).clamp(0.0, 1.0) * 255.0).round() as u8;
    [
        ch(linear[0]),
        ch(linear[1]),
        ch(linear[2]),
        (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

/// A parsed colour: linear-light RGB plus alpha, and whether the source
/// carried an explicit alpha channel.
///
/// The flag matters on the way out: an author who wrote `#FF0000` should get
/// `#RRGGBB` back, not `#RRGGBBFF`. Silently widening every colour would
/// change what a `set_property` event sees and what a diff of two scenes
/// shows.
#[derive(Debug, Clone, Copy)]
struct Rgba {
    /// Linear-light red, green, blue.
    linear: [f32; 3],
    /// Alpha in [0, 1].
    alpha: f32,
    /// The source string carried an alpha channel.
    has_alpha: bool,
}

/// Parse `#RGB`, `#RGBA`, `#RRGGBB`, or `#RRGGBBAA`.
fn parse_hex_color(s: &str) -> Option<Rgba> {
    let s = s.trim_start_matches('#');
    let nibble = |i: usize| -> Option<f32> {
        let c = s.as_bytes().get(i)?;
        let v = (*c as char).to_digit(16)?;
        Some(v as f32 / 15.0)
    };
    let byte = |i: usize| -> Option<f32> {
        let hi = (s.as_bytes().get(i * 2).copied()? as char).to_digit(16)?;
        let lo = (s.as_bytes().get(i * 2 + 1).copied()? as char).to_digit(16)?;
        Some((hi * 16 + lo) as f32 / 255.0)
    };

    let (srgb, alpha, has_alpha) = match s.len() {
        3 => ([nibble(0)?, nibble(1)?, nibble(2)?], 1.0, false),
        4 => ([nibble(0)?, nibble(1)?, nibble(2)?], nibble(3)?, true),
        6 => ([byte(0)?, byte(1)?, byte(2)?], 1.0, false),
        8 => ([byte(0)?, byte(1)?, byte(2)?], byte(3)?, true),
        _ => return None,
    };

    Some(Rgba {
        linear: [
            srgb_to_linear(srgb[0]),
            srgb_to_linear(srgb[1]),
            srgb_to_linear(srgb[2]),
        ],
        alpha,
        has_alpha,
    })
}

/// sRGB transfer function, inverse (encoded value to linear light).
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// sRGB transfer function (linear light to encoded value).
fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Linear-light sRGB to `OKLab`.
///
/// `OKLab` replaces the `CIELAB` conversion this module used before. Both are
/// perceptual, but `CIELAB`'s hue lines bend — interpolating blue to white
/// through it drifts purple — while `OKLab` was fitted specifically so that
/// straight lines look straight. For an engine whose main job is watching one
/// colour become another, that difference is the whole point.
///
/// Reference: Björn Ottosson, "A perceptual color space for image processing".
fn linear_to_oklab(c: [f32; 3]) -> [f32; 3] {
    let l = 0.412_221_47 * c[0] + 0.536_332_55 * c[1] + 0.051_445_995 * c[2];
    let m = 0.211_903_5 * c[0] + 0.680_699_5 * c[1] + 0.107_396_96 * c[2];
    let s = 0.088_302_46 * c[0] + 0.281_718_85 * c[1] + 0.629_978_7 * c[2];

    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();

    [
        0.210_454_26 * l_ + 0.793_617_8 * m_ - 0.004_072_047 * s_,
        1.977_998_5 * l_ - 2.428_592_2 * m_ + 0.450_593_7 * s_,
        0.025_904_037 * l_ + 0.782_771_77 * m_ - 0.808_675_77 * s_,
    ]
}

/// `OKLab` back to linear-light sRGB.
fn oklab_to_linear(lab: [f32; 3]) -> [f32; 3] {
    let l_ = lab[0] + 0.396_337_78 * lab[1] + 0.215_803_76 * lab[2];
    let m_ = lab[0] - 0.105_561_346 * lab[1] - 0.063_854_17 * lab[2];
    let s_ = lab[0] - 0.089_484_18 * lab[1] - 1.291_485_5 * lab[2];

    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;

    [
        4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
        -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
        -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    ]
}

/// Format linear-light RGB plus alpha back to `#RRGGBB` or `#RRGGBBAA`.
fn to_hex(linear: [f32; 3], alpha: f32, with_alpha: bool) -> String {
    let ch = |c: f32| -> u8 {
        let v = linear_to_srgb(c.clamp(0.0, 1.0)).clamp(0.0, 1.0);
        (v * 255.0).round() as u8
    };
    let (r, g, b) = (ch(linear[0]), ch(linear[1]), ch(linear[2]));
    if with_alpha {
        let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    } else {
        format!("#{r:02X}{g:02X}{b:02X}")
    }
}
