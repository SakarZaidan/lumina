use crate::easing::get_easing_fn;
use serde_json::Value;

pub fn interpolate_value(v1: &Value, v2: &Value, t: f32, easing_name: &str) -> Value {
    let t = get_easing_fn(easing_name)(t);

    match (v1, v2) {
        (Value::Number(n1), Value::Number(n2)) => {
            let f1 = n1.as_f64().unwrap() as f32;
            let f2 = n2.as_f64().unwrap() as f32;
            Value::from(f1 + (f2 - f1) * t)
        }
        (Value::Array(a1), Value::Array(a2)) if a1.len() == a2.len() => {
            let mut result = Vec::with_capacity(a1.len());
            for (v1, v2) in a1.iter().zip(a2.iter()) {
                result.push(interpolate_value(v1, v2, t, "linear"));
            }
            Value::Array(result)
        }
        _ => v2.clone(), // Fallback to v2 if types don't match or aren't interpolatable
    }
}
