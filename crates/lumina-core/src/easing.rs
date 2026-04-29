pub type EasingFn = fn(f32) -> f32;

pub fn get_easing_fn(name: &str) -> EasingFn {
    match name {
        "linear" => linear,
        "ease_in_quad" => ease_in_quad,
        "ease_out_quad" => ease_out_quad,
        "ease_in_out_quad" => ease_in_out_quad,
        "ease_in_cubic" => ease_in_cubic,
        "ease_out_cubic" => ease_out_cubic,
        "ease_in_out_cubic" => ease_in_out_cubic,
        _ => linear,
    }
}

pub fn linear(t: f32) -> f32 {
    t
}

pub fn ease_in_quad(t: f32) -> f32 {
    t * t
}

pub fn ease_out_quad(t: f32) -> f32 {
    t * (2.0 - t)
}

pub fn ease_in_out_quad(t: f32) -> f32 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        -1.0 + (4.0 - 2.0 * t) * t
    }
}

pub fn ease_in_cubic(t: f32) -> f32 {
    t * t * t
}

pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t - 1.0;
    t * t * t + 1.0
}

pub fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let t = 2.0 * t - 2.0;
        0.5 * t * t * t + 1.0
    }
}
