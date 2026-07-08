use crate::easing::eval_easing;
use serde_json::Value;

/// Interpolate between two JSON values at normalized progress `t` in `[0, 1]`.
///
/// - Numbers: linear lerp after easing.
/// - Arrays: element-wise interpolation; shorter array is padded by repeating
///   its last element, enabling path morphing between point lists of different
///   lengths.
/// - Hex color strings: interpolated in CIELAB colorspace.
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
            Value::from(f1 + (f2 - f1) * t)
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
            // Interpolate hex colors in CIELAB for perceptually smooth transitions.
            if let (Some(c1), Some(c2)) = (parse_hex_color(s1), parse_hex_color(s2)) {
                let lab1 = rgb_to_lab(c1);
                let lab2 = rgb_to_lab(c2);
                let lab = [
                    lab1[0] + (lab2[0] - lab1[0]) * t,
                    lab1[1] + (lab2[1] - lab1[1]) * t,
                    lab1[2] + (lab2[2] - lab1[2]) * t,
                ];
                return Value::String(lab_to_hex(lab));
            }
            v2.clone()
        }
        _ => v2.clone(),
    }
}

// Parse "#RRGGBB" or "#RGB" into [0.0,1.0] floats
fn parse_hex_color(s: &str) -> Option<[f32; 3]> {
    let s = s.trim_start_matches('#');
    match s.len() {
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()? as f32 / 255.0;
            let g = u8::from_str_radix(&s[2..4], 16).ok()? as f32 / 255.0;
            let b = u8::from_str_radix(&s[4..6], 16).ok()? as f32 / 255.0;
            Some([r, g, b])
        }
        3 => {
            let r = u8::from_str_radix(&s[0..1].repeat(2), 16).ok()? as f32 / 255.0;
            let g = u8::from_str_radix(&s[1..2].repeat(2), 16).ok()? as f32 / 255.0;
            let b = u8::from_str_radix(&s[2..3].repeat(2), 16).ok()? as f32 / 255.0;
            Some([r, g, b])
        }
        _ => None,
    }
}

// sRGB → linear RGB → XYZ D65 → CIELAB
fn rgb_to_lab(rgb: [f32; 3]) -> [f32; 3] {
    // sRGB gamma expand
    let linearize = |c: f32| -> f32 {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let r = linearize(rgb[0]);
    let g = linearize(rgb[1]);
    let b = linearize(rgb[2]);

    // RGB → XYZ (D65)
    let x = r * 0.4124564 + g * 0.3575761 + b * 0.1804375;
    let y = r * 0.2126729 + g * 0.7151522 + b * 0.0721750;
    let z = r * 0.019_334 + g * 0.119_192 + b * 0.950_304;

    // XYZ → LAB (D65 white: Xn=0.95047, Yn=1.0, Zn=1.08883)
    let f = |t: f32| -> f32 {
        if t > 0.008856 {
            t.powf(1.0 / 3.0)
        } else {
            7.787 * t + 16.0 / 116.0
        }
    };
    let fx = f(x / 0.95047);
    let fy = f(y / 1.00000);
    let fz = f(z / 1.08883);

    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

// CIELAB → XYZ D65 → linear RGB → sRGB → "#RRGGBB"
fn lab_to_hex(lab: [f32; 3]) -> String {
    let fy = (lab[0] + 16.0) / 116.0;
    let fx = lab[1] / 500.0 + fy;
    let fz = fy - lab[2] / 200.0;

    let f_inv = |t: f32| -> f32 {
        let t3 = t * t * t;
        if t3 > 0.008856 {
            t3
        } else {
            (t - 16.0 / 116.0) / 7.787
        }
    };

    let x = f_inv(fx) * 0.95047;
    let y = f_inv(fy) * 1.00000;
    let z = f_inv(fz) * 1.08883;

    // XYZ → linear RGB
    let rl = x * 3.2404542 - y * 1.5371385 - z * 0.4985314;
    let gl = -x * 0.969_266 + y * 1.876_011 + z * 0.041_556;
    let bl = x * 0.0556434 - y * 0.2040259 + z * 1.0572252;

    // linear → sRGB gamma compress
    let compress = |c: f32| -> u8 {
        let c = c.clamp(0.0, 1.0);
        let srgb = if c <= 0.0031308 {
            12.92 * c
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        };
        (srgb * 255.0).round() as u8
    };

    format!(
        "#{:02X}{:02X}{:02X}",
        compress(rl),
        compress(gl),
        compress(bl)
    )
}
