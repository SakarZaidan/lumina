#[cfg(test)]
mod tests {
    use crate::{Renderer, skia_backend::SkiaRenderer};
    use lumina_schema::{CircleProps, LineProps, Object, RectangleProps};
    use serde_json::{json, Value};
    use std::collections::HashMap;

    fn make_renderer() -> SkiaRenderer {
        SkiaRenderer::new()
    }

    fn render(
        renderer: &mut SkiaRenderer,
        objects: HashMap<String, Object>,
        states: HashMap<String, Value>,
        w: u32, h: u32,
        bg: &str,
    ) -> Vec<u8> {
        renderer.render_frame(&objects, &states, w, h, bg, None).expect("render_frame failed")
    }

    fn pixel_at(data: &[u8], x: u32, y: u32, width: u32) -> (u8, u8, u8, u8) {
        let idx = ((y * width + x) * 4) as usize;
        (data[idx], data[idx + 1], data[idx + 2], data[idx + 3])
    }

    #[test]
    fn test_render_empty_scene_correct_size() {
        let mut r = make_renderer();
        let data = render(&mut r, HashMap::new(), HashMap::new(), 100, 80, "#000000");
        // RGBA = 4 bytes per pixel
        assert_eq!(data.len(), 100 * 80 * 4, "Frame data size should match width*height*4");
    }

    #[test]
    fn test_background_color_applied() {
        let mut r = make_renderer();
        let data = render(&mut r, HashMap::new(), HashMap::new(), 10, 10, "#FF0000");
        let (red, green, blue, _alpha) = pixel_at(&data, 0, 0, 10);
        assert_eq!(red, 255, "Red channel should be 255 for #FF0000 background");
        assert_eq!(green, 0, "Green channel should be 0");
        assert_eq!(blue, 0, "Blue channel should be 0");
    }

    #[test]
    fn test_background_short_hex_parsed() {
        let mut r = make_renderer();
        // #000 should parse as #000000 (black)
        let data = render(&mut r, HashMap::new(), HashMap::new(), 4, 4, "#000");
        let (red, green, blue, _) = pixel_at(&data, 0, 0, 4);
        assert_eq!((red, green, blue), (0, 0, 0), "Short hex #000 should produce black background");
    }

    #[test]
    fn test_render_is_deterministic() {
        let mut objects = HashMap::new();
        objects.insert("c".into(), Object::Circle(CircleProps {
            cx: 50.0, cy: 50.0, radius: 20.0,
            z_index: 1, fill: "#FF0000".into(), stroke: None, stroke_width: 0.0, opacity: 1.0,
        }));
        let mut states = HashMap::new();
        states.insert("c".into(), json!({
            "cx": 50.0, "cy": 50.0, "radius": 20.0,
            "fill": "#FF0000", "opacity": 1.0, "stroke_width": 0.0
        }));

        let mut r1 = make_renderer();
        let frame1 = render(&mut r1, objects.clone(), states.clone(), 100, 100, "#000000");
        let mut r2 = make_renderer();
        let frame2 = render(&mut r2, objects, states, 100, 100, "#000000");

        assert_eq!(frame1, frame2, "Same inputs should produce identical pixel output");
    }

    #[test]
    fn test_circle_center_pixel_matches_fill() {
        let mut objects = HashMap::new();
        objects.insert("c".into(), Object::Circle(CircleProps {
            cx: 50.0, cy: 50.0, radius: 30.0,
            z_index: 1, fill: "#FFFFFF".into(), stroke: None, stroke_width: 0.0, opacity: 1.0,
        }));
        let mut states = HashMap::new();
        states.insert("c".into(), json!({
            "cx": 50.0, "cy": 50.0, "radius": 30.0,
            "fill": "#FFFFFF", "opacity": 1.0, "stroke_width": 0.0
        }));

        let mut r = make_renderer();
        let data = render(&mut r, objects, states, 100, 100, "#000000");

        let (red, green, blue, _) = pixel_at(&data, 50, 50, 100);
        assert_eq!(red, 255, "Circle center should be white (red=255)");
        assert_eq!(green, 255, "Circle center should be white (green=255)");
        assert_eq!(blue, 255, "Circle center should be white (blue=255)");

        // Corner should still be background (black)
        let (cr, cg, cb, _) = pixel_at(&data, 0, 0, 100);
        assert_eq!((cr, cg, cb), (0, 0, 0), "Corner should remain black (background)");
    }

    #[test]
    fn test_opacity_zero_renders_as_background() {
        let mut objects = HashMap::new();
        objects.insert("c".into(), Object::Circle(CircleProps {
            cx: 50.0, cy: 50.0, radius: 40.0,
            z_index: 1, fill: "#FFFFFF".into(), stroke: None, stroke_width: 0.0, opacity: 0.0,
        }));
        let mut states = HashMap::new();
        states.insert("c".into(), json!({
            "cx": 50.0, "cy": 50.0, "radius": 40.0,
            "fill": "#FFFFFF", "opacity": 0.0, "stroke_width": 0.0
        }));

        let mut r = make_renderer();
        let data = render(&mut r, objects, states, 100, 100, "#000000");

        // Center pixel should still be black (circle is transparent)
        let (red, green, blue, _) = pixel_at(&data, 50, 50, 100);
        assert_eq!((red, green, blue), (0, 0, 0), "Transparent circle should not change background");
    }

    #[test]
    fn test_z_index_determines_draw_order() {
        // Two overlapping circles at same position: red z=1, blue z=2
        // Blue (higher z) should win at the center pixel
        let mut objects = HashMap::new();
        objects.insert("red".into(), Object::Circle(CircleProps {
            cx: 50.0, cy: 50.0, radius: 30.0,
            z_index: 1, fill: "#FF0000".into(), stroke: None, stroke_width: 0.0, opacity: 1.0,
        }));
        objects.insert("blue".into(), Object::Circle(CircleProps {
            cx: 50.0, cy: 50.0, radius: 30.0,
            z_index: 2, fill: "#0000FF".into(), stroke: None, stroke_width: 0.0, opacity: 1.0,
        }));
        let mut states = HashMap::new();
        states.insert("red".into(), json!({
            "cx": 50.0, "cy": 50.0, "radius": 30.0,
            "fill": "#FF0000", "opacity": 1.0, "stroke_width": 0.0
        }));
        states.insert("blue".into(), json!({
            "cx": 50.0, "cy": 50.0, "radius": 30.0,
            "fill": "#0000FF", "opacity": 1.0, "stroke_width": 0.0
        }));

        let mut r = make_renderer();
        let data = render(&mut r, objects, states, 100, 100, "#000000");

        let (red, _green, blue, _) = pixel_at(&data, 50, 50, 100);
        assert!(blue > red, "Blue (z=2) should be on top of red (z=1) at center pixel");
    }

    #[test]
    fn test_rectangle_renders_correctly() {
        let mut objects = HashMap::new();
        objects.insert("rect".into(), Object::Rectangle(RectangleProps {
            x: 10.0, y: 10.0, width: 80.0, height: 80.0,
            z_index: 1, fill: "#00FF00".into(), stroke: None, stroke_width: 0.0, opacity: 1.0,
        }));
        let mut states = HashMap::new();
        states.insert("rect".into(), json!({
            "x": 10.0, "y": 10.0, "width": 80.0, "height": 80.0,
            "fill": "#00FF00", "opacity": 1.0, "stroke_width": 0.0
        }));

        let mut r = make_renderer();
        let data = render(&mut r, objects, states, 100, 100, "#000000");

        // Center should be green
        let (_, green, _, _) = pixel_at(&data, 50, 50, 100);
        assert!(green > 200, "Rectangle center should be green-ish, got green={green}");

        // Corner outside rect should be black
        let (cr, cg, cb, _) = pixel_at(&data, 1, 1, 100);
        assert_eq!((cr, cg, cb), (0, 0, 0), "Outside rectangle should be black");
    }

    #[test]
    fn test_draw_fraction_zero_hides_line() {
        // A line with draw_fraction=0 should render nothing (background stays black)
        let mut objects = HashMap::new();
        objects.insert("l".into(), Object::Line(LineProps {
            x1: 0.0, y1: 50.0, x2: 100.0, y2: 50.0,
            z_index: 1, stroke: "#FFFFFF".into(), stroke_width: 4.0,
            dash: None, draw_fraction: Some(0.0), opacity: 1.0,
        }));
        let mut states = HashMap::new();
        states.insert("l".into(), json!({
            "x1": 0.0, "y1": 50.0, "x2": 100.0, "y2": 50.0,
            "stroke": "#FFFFFF", "stroke_width": 4.0, "opacity": 1.0,
            "draw_fraction": 0.0
        }));

        let mut r = make_renderer();
        let data = render(&mut r, objects, states, 100, 100, "#000000");

        // Center of line at (50, 50) should remain black
        let (red, green, blue, _) = pixel_at(&data, 50, 50, 100);
        assert_eq!((red, green, blue), (0, 0, 0), "Line with draw_fraction=0 should be invisible");
    }

    #[test]
    fn test_draw_fraction_one_draws_full_line() {
        let mut objects = HashMap::new();
        objects.insert("l".into(), Object::Line(LineProps {
            x1: 10.0, y1: 50.0, x2: 90.0, y2: 50.0,
            z_index: 1, stroke: "#FFFFFF".into(), stroke_width: 4.0,
            dash: None, draw_fraction: Some(1.0), opacity: 1.0,
        }));
        let mut states = HashMap::new();
        states.insert("l".into(), json!({
            "x1": 10.0, "y1": 50.0, "x2": 90.0, "y2": 50.0,
            "stroke": "#FFFFFF", "stroke_width": 4.0, "opacity": 1.0,
            "draw_fraction": 1.0
        }));

        let mut r = make_renderer();
        let data = render(&mut r, objects, states, 100, 100, "#000000");

        // Center of line at (50, 50) should be white
        let (red, green, blue, _) = pixel_at(&data, 50, 50, 100);
        assert!(red > 200 && green > 200 && blue > 200, "Line with draw_fraction=1 should be visible");
    }

    #[test]
    fn test_missing_object_state_returns_error() {
        let mut objects = HashMap::new();
        objects.insert("c".into(), Object::Circle(CircleProps {
            cx: 50.0, cy: 50.0, radius: 20.0,
            z_index: 0, fill: "#FFF".into(), stroke: None, stroke_width: 0.0, opacity: 1.0,
        }));
        // states is intentionally empty
        let states = HashMap::new();

        let mut r = make_renderer();
        let result = r.render_frame(&objects, &states, 100, 100, "#000000", None);
        assert!(result.is_err(), "render_frame should return Err when object has no state");
    }
}
