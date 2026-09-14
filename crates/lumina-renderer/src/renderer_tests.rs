#[cfg(test)]
mod tests {
    use crate::{skia_backend::SkiaRenderer, Renderer};
    use luminafx_schema::{
        CircleProps, ImageProps, LineProps, Object, ParticlesProps, RectangleProps, SVGProps,
    };
    use serde_json::{json, Value};
    use std::collections::HashMap;

    fn png_bytes(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let mut img = image::RgbaImage::new(w, h);
        for p in img.pixels_mut() {
            *p = image::Rgba(rgba);
        }
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .expect("png encode");
        buf
    }

    fn two_frame_gif() -> Vec<u8> {
        use image::codecs::gif::GifEncoder;
        use image::{Delay, Frame};
        let mut red = image::RgbaImage::new(8, 8);
        for p in red.pixels_mut() {
            *p = image::Rgba([255, 0, 0, 255]);
        }
        let mut blue = image::RgbaImage::new(8, 8);
        for p in blue.pixels_mut() {
            *p = image::Rgba([0, 0, 255, 255]);
        }
        let mut buf = Vec::new();
        {
            let mut enc = GifEncoder::new(&mut buf);
            enc.encode_frame(Frame::from_parts(
                red,
                0,
                0,
                Delay::from_numer_denom_ms(100, 1),
            ))
            .expect("gif frame 0");
            enc.encode_frame(Frame::from_parts(
                blue,
                0,
                0,
                Delay::from_numer_denom_ms(100, 1),
            ))
            .expect("gif frame 1");
        }
        buf
    }

    fn make_renderer() -> SkiaRenderer {
        SkiaRenderer::new()
    }

    // Taken by value so call sites can build maps inline and hand them over;
    // borrowing here would add `&` to sixteen call sites for no benefit in a
    // helper that owns nothing afterwards.
    #[allow(clippy::needless_pass_by_value)]
    fn render(
        renderer: &mut SkiaRenderer,
        objects: HashMap<String, Object>,
        states: HashMap<String, Value>,
        w: u32,
        h: u32,
        bg: &str,
    ) -> Vec<u8> {
        renderer
            .render_frame(&objects, &states, w, h, bg, None)
            .expect("render_frame failed")
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
        assert_eq!(
            data.len(),
            100 * 80 * 4,
            "Frame data size should match width*height*4"
        );
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
        assert_eq!(
            (red, green, blue),
            (0, 0, 0),
            "Short hex #000 should produce black background"
        );
    }

    #[test]
    fn test_render_is_deterministic() {
        let mut objects = HashMap::new();
        objects.insert(
            "c".into(),
            Object::Circle(CircleProps {
                cx: 50.0,
                cy: 50.0,
                radius: 20.0,
                z_index: 1,
                fill: "#FF0000".into(),
                stroke: None,
                stroke_width: 0.0,
                shadow: None,
                opacity: 1.0,
            }),
        );
        let mut states = HashMap::new();
        states.insert(
            "c".into(),
            json!({
                "cx": 50.0, "cy": 50.0, "radius": 20.0,
                "fill": "#FF0000", "opacity": 1.0, "stroke_width": 0.0
            }),
        );

        let mut r1 = make_renderer();
        let frame1 = render(
            &mut r1,
            objects.clone(),
            states.clone(),
            100,
            100,
            "#000000",
        );
        let mut r2 = make_renderer();
        let frame2 = render(&mut r2, objects, states, 100, 100, "#000000");

        assert_eq!(
            frame1, frame2,
            "Same inputs should produce identical pixel output"
        );
    }

    #[test]
    fn test_circle_center_pixel_matches_fill() {
        let mut objects = HashMap::new();
        objects.insert(
            "c".into(),
            Object::Circle(CircleProps {
                cx: 50.0,
                cy: 50.0,
                radius: 30.0,
                z_index: 1,
                fill: "#FFFFFF".into(),
                stroke: None,
                stroke_width: 0.0,
                shadow: None,
                opacity: 1.0,
            }),
        );
        let mut states = HashMap::new();
        states.insert(
            "c".into(),
            json!({
                "cx": 50.0, "cy": 50.0, "radius": 30.0,
                "fill": "#FFFFFF", "opacity": 1.0, "stroke_width": 0.0
            }),
        );

        let mut r = make_renderer();
        let data = render(&mut r, objects, states, 100, 100, "#000000");

        let (red, green, blue, _) = pixel_at(&data, 50, 50, 100);
        assert_eq!(red, 255, "Circle center should be white (red=255)");
        assert_eq!(green, 255, "Circle center should be white (green=255)");
        assert_eq!(blue, 255, "Circle center should be white (blue=255)");

        // Corner should still be background (black)
        let (cr, cg, cb, _) = pixel_at(&data, 0, 0, 100);
        assert_eq!(
            (cr, cg, cb),
            (0, 0, 0),
            "Corner should remain black (background)"
        );
    }

    #[test]
    fn test_opacity_zero_renders_as_background() {
        let mut objects = HashMap::new();
        objects.insert(
            "c".into(),
            Object::Circle(CircleProps {
                cx: 50.0,
                cy: 50.0,
                radius: 40.0,
                z_index: 1,
                fill: "#FFFFFF".into(),
                stroke: None,
                stroke_width: 0.0,
                shadow: None,
                opacity: 0.0,
            }),
        );
        let mut states = HashMap::new();
        states.insert(
            "c".into(),
            json!({
                "cx": 50.0, "cy": 50.0, "radius": 40.0,
                "fill": "#FFFFFF", "opacity": 0.0, "stroke_width": 0.0
            }),
        );

        let mut r = make_renderer();
        let data = render(&mut r, objects, states, 100, 100, "#000000");

        // Center pixel should still be black (circle is transparent)
        let (red, green, blue, _) = pixel_at(&data, 50, 50, 100);
        assert_eq!(
            (red, green, blue),
            (0, 0, 0),
            "Transparent circle should not change background"
        );
    }

    #[test]
    fn test_z_index_determines_draw_order() {
        // Two overlapping circles at same position: red z=1, blue z=2
        // Blue (higher z) should win at the center pixel
        let mut objects = HashMap::new();
        objects.insert(
            "red".into(),
            Object::Circle(CircleProps {
                cx: 50.0,
                cy: 50.0,
                radius: 30.0,
                z_index: 1,
                fill: "#FF0000".into(),
                stroke: None,
                stroke_width: 0.0,
                shadow: None,
                opacity: 1.0,
            }),
        );
        objects.insert(
            "blue".into(),
            Object::Circle(CircleProps {
                cx: 50.0,
                cy: 50.0,
                radius: 30.0,
                z_index: 2,
                fill: "#0000FF".into(),
                stroke: None,
                stroke_width: 0.0,
                shadow: None,
                opacity: 1.0,
            }),
        );
        let mut states = HashMap::new();
        states.insert(
            "red".into(),
            json!({
                "cx": 50.0, "cy": 50.0, "radius": 30.0,
                "fill": "#FF0000", "opacity": 1.0, "stroke_width": 0.0
            }),
        );
        states.insert(
            "blue".into(),
            json!({
                "cx": 50.0, "cy": 50.0, "radius": 30.0,
                "fill": "#0000FF", "opacity": 1.0, "stroke_width": 0.0
            }),
        );

        let mut r = make_renderer();
        let data = render(&mut r, objects, states, 100, 100, "#000000");

        let (red, _green, blue, _) = pixel_at(&data, 50, 50, 100);
        assert!(
            blue > red,
            "Blue (z=2) should be on top of red (z=1) at center pixel"
        );
    }

    #[test]
    fn test_rectangle_renders_correctly() {
        let mut objects = HashMap::new();
        objects.insert(
            "rect".into(),
            Object::Rectangle(RectangleProps {
                x: 10.0,
                y: 10.0,
                width: 80.0,
                height: 80.0,
                z_index: 1,
                fill: "#00FF00".into(),
                stroke: None,
                stroke_width: 0.0,
                rx: 0.0,
                ry: 0.0,
                shadow: None,
                opacity: 1.0,
            }),
        );
        let mut states = HashMap::new();
        states.insert(
            "rect".into(),
            json!({
                "x": 10.0, "y": 10.0, "width": 80.0, "height": 80.0,
                "fill": "#00FF00", "opacity": 1.0, "stroke_width": 0.0
            }),
        );

        let mut r = make_renderer();
        let data = render(&mut r, objects, states, 100, 100, "#000000");

        // Center should be green
        let (_, green, _, _) = pixel_at(&data, 50, 50, 100);
        assert!(
            green > 200,
            "Rectangle center should be green-ish, got green={green}"
        );

        // Corner outside rect should be black
        let (cr, cg, cb, _) = pixel_at(&data, 1, 1, 100);
        assert_eq!((cr, cg, cb), (0, 0, 0), "Outside rectangle should be black");
    }

    #[test]
    fn test_draw_fraction_zero_hides_line() {
        // A line with draw_fraction=0 should render nothing (background stays black)
        let mut objects = HashMap::new();
        objects.insert(
            "l".into(),
            Object::Line(LineProps {
                x1: 0.0,
                y1: 50.0,
                x2: 100.0,
                y2: 50.0,
                z_index: 1,
                stroke: "#FFFFFF".into(),
                stroke_width: 4.0,
                dash: None,
                draw_fraction: Some(0.0),
                opacity: 1.0,
            }),
        );
        let mut states = HashMap::new();
        states.insert(
            "l".into(),
            json!({
                "x1": 0.0, "y1": 50.0, "x2": 100.0, "y2": 50.0,
                "stroke": "#FFFFFF", "stroke_width": 4.0, "opacity": 1.0,
                "draw_fraction": 0.0
            }),
        );

        let mut r = make_renderer();
        let data = render(&mut r, objects, states, 100, 100, "#000000");

        // Center of line at (50, 50) should remain black
        let (red, green, blue, _) = pixel_at(&data, 50, 50, 100);
        assert_eq!(
            (red, green, blue),
            (0, 0, 0),
            "Line with draw_fraction=0 should be invisible"
        );
    }

    #[test]
    fn test_draw_fraction_one_draws_full_line() {
        let mut objects = HashMap::new();
        objects.insert(
            "l".into(),
            Object::Line(LineProps {
                x1: 10.0,
                y1: 50.0,
                x2: 90.0,
                y2: 50.0,
                z_index: 1,
                stroke: "#FFFFFF".into(),
                stroke_width: 4.0,
                dash: None,
                draw_fraction: Some(1.0),
                opacity: 1.0,
            }),
        );
        let mut states = HashMap::new();
        states.insert(
            "l".into(),
            json!({
                "x1": 10.0, "y1": 50.0, "x2": 90.0, "y2": 50.0,
                "stroke": "#FFFFFF", "stroke_width": 4.0, "opacity": 1.0,
                "draw_fraction": 1.0
            }),
        );

        let mut r = make_renderer();
        let data = render(&mut r, objects, states, 100, 100, "#000000");

        // Center of line at (50, 50) should be white
        let (red, green, blue, _) = pixel_at(&data, 50, 50, 100);
        assert!(
            red > 200 && green > 200 && blue > 200,
            "Line with draw_fraction=1 should be visible"
        );
    }

    #[test]
    fn test_image_composites_onto_frame() {
        let mut r = make_renderer();
        r.load_image("logo", &png_bytes(8, 8, [255, 0, 0, 255]))
            .expect("load image");

        let mut objects = HashMap::new();
        objects.insert(
            "img".into(),
            Object::Image(ImageProps {
                asset_id: "logo".into(),
                x: 0.0,
                y: 0.0,
                width: Some(100.0),
                height: Some(100.0),
                rotation: 0.0,
                z_index: 1,
                opacity: 1.0,
            }),
        );
        let mut states = HashMap::new();
        states.insert(
            "img".into(),
            json!({
                "asset_id": "logo", "x": 0.0, "y": 0.0,
                "width": 100.0, "height": 100.0, "opacity": 1.0, "rotation": 0.0
            }),
        );

        let data = render(&mut r, objects, states, 100, 100, "#000000");
        let (red, green, blue, _) = pixel_at(&data, 50, 50, 100);
        assert!(
            red > 200 && green < 60 && blue < 60,
            "image center should be red, got ({red},{green},{blue})"
        );
    }

    #[test]
    fn test_image_opacity_blends_with_background() {
        let mut r = make_renderer();
        r.load_image("logo", &png_bytes(8, 8, [255, 255, 255, 255]))
            .expect("load image");

        let mut objects = HashMap::new();
        objects.insert(
            "img".into(),
            Object::Image(ImageProps {
                asset_id: "logo".into(),
                x: 0.0,
                y: 0.0,
                width: Some(100.0),
                height: Some(100.0),
                rotation: 0.0,
                z_index: 1,
                opacity: 0.5,
            }),
        );
        let mut states = HashMap::new();
        states.insert(
            "img".into(),
            json!({
                "asset_id": "logo", "x": 0.0, "y": 0.0,
                "width": 100.0, "height": 100.0, "opacity": 0.5, "rotation": 0.0
            }),
        );

        let data = render(&mut r, objects, states, 100, 100, "#000000");
        let (red, _, _, _) = pixel_at(&data, 50, 50, 100);
        assert!(
            (100..=160).contains(&red),
            "white image at 0.5 opacity over black should be ~128, got {red}"
        );
    }

    #[test]
    fn test_animated_gif_advances_with_time() {
        let mut r = make_renderer();
        r.load_image("anim", &two_frame_gif()).expect("load gif");

        let mut objects = HashMap::new();
        objects.insert(
            "g".into(),
            Object::Image(ImageProps {
                asset_id: "anim".into(),
                x: 0.0,
                y: 0.0,
                width: Some(100.0),
                height: Some(100.0),
                rotation: 0.0,
                z_index: 1,
                opacity: 1.0,
            }),
        );
        let mut states = HashMap::new();
        states.insert(
            "g".into(),
            json!({
                "asset_id": "anim", "x": 0.0, "y": 0.0,
                "width": 100.0, "height": 100.0, "opacity": 1.0, "rotation": 0.0
            }),
        );

        // Frame 0 (0–100ms) is red; frame 1 (100–200ms) is blue.
        r.set_time(0.0);
        let f0 = r
            .render_frame(&objects, &states, 100, 100, "#000000", None)
            .unwrap();
        let (r0, _, b0, _) = pixel_at(&f0, 50, 50, 100);
        assert!(
            r0 > b0,
            "at t=0 GIF should show red frame, got r={r0} b={b0}"
        );

        r.set_time(0.15);
        let f1 = r
            .render_frame(&objects, &states, 100, 100, "#000000", None)
            .unwrap();
        let (r1, _, b1, _) = pixel_at(&f1, 50, 50, 100);
        assert!(
            b1 > r1,
            "at t=0.15 GIF should show blue frame, got r={r1} b={b1}"
        );
    }

    #[test]
    fn test_svg_rasterizes_and_composites() {
        let mut r = make_renderer();
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="#00FF00"/></svg>"##;
        r.load_image("icon", svg).expect("load svg");

        let mut objects = HashMap::new();
        objects.insert(
            "s".into(),
            Object::SVG(SVGProps {
                asset_id: "icon".into(),
                x: 0.0,
                y: 0.0,
                width: Some(100.0),
                height: Some(100.0),
                rotation: 0.0,
                z_index: 1,
                opacity: 1.0,
            }),
        );
        let mut states = HashMap::new();
        states.insert(
            "s".into(),
            json!({
                "asset_id": "icon", "x": 0.0, "y": 0.0,
                "width": 100.0, "height": 100.0, "opacity": 1.0, "rotation": 0.0
            }),
        );

        let data = render(&mut r, objects, states, 100, 100, "#000000");
        let (red, green, blue, _) = pixel_at(&data, 50, 50, 100);
        assert!(
            green > 200 && red < 60 && blue < 60,
            "svg center should be green, got ({red},{green},{blue})"
        );
    }

    fn rect_object() -> Object {
        Object::Rectangle(RectangleProps {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            z_index: 1,
            fill: "#FFFFFF".into(),
            stroke: None,
            stroke_width: 0.0,
            rx: 0.0,
            ry: 0.0,
            shadow: None,
            opacity: 1.0,
        })
    }

    #[test]
    fn test_linear_gradient_fill_transitions_red_to_blue() {
        let mut objects = HashMap::new();
        objects.insert("r".into(), rect_object());
        let mut states = HashMap::new();
        states.insert("r".into(), json!({
            "x": 0.0, "y": 0.0, "width": 100.0, "height": 100.0, "opacity": 1.0,
            "fill": { "type": "linear", "stops": [[0.0, "#FF0000"], [1.0, "#0000FF"]], "angle": 0.0 }
        }));

        let mut r = make_renderer();
        let data = render(&mut r, objects, states, 100, 100, "#000000");
        let (lr, _, lb, _) = pixel_at(&data, 6, 50, 100);
        let (rr, _, rb, _) = pixel_at(&data, 93, 50, 100);
        assert!(
            lr > lb,
            "left edge should be red-dominant, got r={lr} b={lb}"
        );
        assert!(
            rb > rr,
            "right edge should be blue-dominant, got r={rr} b={rb}"
        );
    }

    #[test]
    fn test_rounded_rectangle_clips_corner() {
        let mut objects = HashMap::new();
        objects.insert("r".into(), rect_object());
        let mut states = HashMap::new();
        states.insert(
            "r".into(),
            json!({
                "x": 0.0, "y": 0.0, "width": 100.0, "height": 100.0, "opacity": 1.0,
                "fill": "#FFFFFF", "rx": 40.0, "ry": 40.0
            }),
        );

        let mut r = make_renderer();
        let data = render(&mut r, objects, states, 100, 100, "#000000");
        // Corner is cut away by the radius → background.
        let (cr, cg, cb, _) = pixel_at(&data, 1, 1, 100);
        assert_eq!(
            (cr, cg, cb),
            (0, 0, 0),
            "rounded corner should be background"
        );
        // Center is filled.
        let (mr, mg, mb, _) = pixel_at(&data, 50, 50, 100);
        assert_eq!(
            (mr, mg, mb),
            (255, 255, 255),
            "center should be filled white"
        );
    }

    #[test]
    fn test_drop_shadow_darkens_outside_shape() {
        let mut objects = HashMap::new();
        objects.insert("r".into(), rect_object());
        let mut states = HashMap::new();
        // 20x20 green rect centered at (40..60), black blurred shadow over white bg.
        states.insert(
            "r".into(),
            json!({
                "x": 40.0, "y": 40.0, "width": 20.0, "height": 20.0, "opacity": 1.0,
                "fill": "#00FF00",
                "shadow": { "color": "#000000", "blur": 8.0, "dx": 0.0, "dy": 0.0, "opacity": 1.0 }
            }),
        );

        let mut r = make_renderer();
        let data = render(&mut r, objects, states, 100, 100, "#FFFFFF");
        // Just outside the rect's left edge, within the blur radius: darkened by shadow.
        let (sr, sg, sb, _) = pixel_at(&data, 34, 50, 100);
        assert!(
            sr < 240 && sg < 240 && sb < 240,
            "shadow should darken outside the shape, got ({sr},{sg},{sb})"
        );
        // Far corner stays white.
        let (fr, fg, fb, _) = pixel_at(&data, 2, 2, 100);
        assert_eq!(
            (fr, fg, fb),
            (255, 255, 255),
            "far corner should remain background white"
        );
    }

    #[test]
    fn test_particles_render_and_are_deterministic() {
        let mut objects = HashMap::new();
        objects.insert(
            "p".into(),
            Object::Particles(ParticlesProps {
                count: 200,
                emitter_x: 50.0,
                emitter_y: 50.0,
                lifetime: 2.0,
                speed: 40.0,
                spread: 360.0,
                size: 3.0,
                color: "#FFFFFF".into(),
                z_index: 1,
                opacity: 1.0,
            }),
        );
        let mut states = HashMap::new();
        states.insert(
            "p".into(),
            json!({
                "count": 200, "emitter_x": 50.0, "emitter_y": 50.0,
                "lifetime": 2.0, "speed": 40.0, "spread": 360.0, "size": 3.0,
                "color": "#FFFFFF", "opacity": 1.0
            }),
        );

        let mut r1 = make_renderer();
        r1.set_time(0.5);
        let f1 = r1
            .render_frame(&objects, &states, 100, 100, "#000000", None)
            .unwrap();
        // Some pixels must be lit by particles.
        let lit = f1
            .chunks(4)
            .filter(|px| px[0] > 10 || px[1] > 10 || px[2] > 10)
            .count();
        assert!(lit > 0, "particles should light up some pixels");

        // Deterministic: same time → identical frame.
        let mut r2 = make_renderer();
        r2.set_time(0.5);
        let f2 = r2
            .render_frame(&objects, &states, 100, 100, "#000000", None)
            .unwrap();
        assert_eq!(
            f1, f2,
            "particle rendering must be deterministic at a fixed time"
        );
    }

    // ── Vello GPU backend parity ────────────────────────────────────────────
    // These render on the CPU-fallback Vello backend. If no GPU/CPU adapter is
    // available in the environment, construction fails and the test is skipped.

    #[test]
    fn test_vello_particles_render() {
        use crate::vello_backend::VelloRenderer;
        let mut r = match VelloRenderer::new() {
            Ok(r) => r,
            Err(_) => return, // no adapter in this environment — skip
        };
        let mut objects = HashMap::new();
        objects.insert(
            "p".into(),
            Object::Particles(ParticlesProps {
                count: 300,
                emitter_x: 100.0,
                emitter_y: 100.0,
                lifetime: 2.0,
                speed: 40.0,
                spread: 360.0,
                size: 4.0,
                color: "#FFFFFF".into(),
                z_index: 1,
                opacity: 1.0,
            }),
        );
        let mut states = HashMap::new();
        states.insert(
            "p".into(),
            json!({
                "count": 300, "emitter_x": 100.0, "emitter_y": 100.0,
                "lifetime": 2.0, "speed": 40.0, "spread": 360.0, "size": 4.0,
                "color": "#FFFFFF", "opacity": 1.0
            }),
        );
        r.set_time(0.5);
        let frame = r
            .render_frame(&objects, &states, 200, 200, "#000000", None)
            .expect("vello render");
        let lit = frame
            .chunks(4)
            .filter(|px| px[0] > 10 || px[1] > 10 || px[2] > 10)
            .count();
        assert!(lit > 0, "vello particles should light up some pixels");
    }

    #[test]
    fn test_vello_image_composites() {
        use crate::vello_backend::VelloRenderer;
        let mut r = match VelloRenderer::new() {
            Ok(r) => r,
            Err(_) => return,
        };
        r.load_image("logo", &png_bytes(16, 16, [255, 0, 0, 255]))
            .expect("load image into vello");
        let mut objects = HashMap::new();
        objects.insert(
            "img".into(),
            Object::Image(ImageProps {
                asset_id: "logo".into(),
                x: 20.0,
                y: 20.0,
                width: Some(40.0),
                height: Some(40.0),
                rotation: 0.0,
                z_index: 1,
                opacity: 1.0,
            }),
        );
        let mut states = HashMap::new();
        states.insert(
            "img".into(),
            json!({ "asset_id": "logo", "x": 20.0, "y": 20.0, "width": 40.0, "height": 40.0, "rotation": 0.0, "opacity": 1.0 }),
        );
        let frame = r
            .render_frame(&objects, &states, 100, 100, "#000000", None)
            .expect("vello render");
        // The red square should produce strongly-red, low-blue pixels somewhere.
        let red_pixels = frame
            .chunks(4)
            .filter(|px| px[0] > 150 && px[2] < 80)
            .count();
        assert!(
            red_pixels > 100,
            "vello should composite the red image (found {red_pixels} red px)"
        );
    }

    // ── LaTeX → Unicode conversion ──────────────────────────────────────────
    #[test]
    fn test_latex_unicode_superscripts_and_greek() {
        use crate::skia_backend::latex_to_unicode;
        assert_eq!(latex_to_unicode(r"E = mc^2"), "E = mc²");
        assert_eq!(latex_to_unicode(r"a^2 + b^2 = c^2"), "a² + b² = c²");
        assert_eq!(latex_to_unicode(r"e^x"), "eˣ");
        assert_eq!(latex_to_unicode(r"\pi r^2"), "π r²");
    }

    #[test]
    fn test_latex_unicode_frac_and_spacing() {
        use crate::skia_backend::latex_to_unicode;
        assert_eq!(latex_to_unicode(r"\frac{d}{dx}"), "d/dx");
        assert_eq!(latex_to_unicode(r"\frac{\pi^2}{6}"), "π²/6");
        // Spacing command collapses; \sum becomes Σ.
        assert_eq!(latex_to_unicode(r"\sum \frac{1}{n^2}"), "Σ 1/n²");
        // Multi-term numerator gets wrapped for clarity.
        assert_eq!(latex_to_unicode(r"\frac{a+b}{2}"), "(a+b)/2");
    }

    #[test]
    fn test_latex_unicode_subscripts() {
        use crate::skia_backend::latex_to_unicode;
        assert_eq!(latex_to_unicode(r"x_0 + x_1"), "x₀ + x₁");
        assert_eq!(latex_to_unicode(r"a_{n}"), "aₙ");
    }

    #[test]
    fn test_latex_unicode_strips_commands_no_leak() {
        use crate::skia_backend::latex_to_unicode;
        // \vec is handled (dropped); unknown commands never leak literally.
        assert_eq!(latex_to_unicode(r"\vec{v}"), "v");
        assert_eq!(latex_to_unicode(r"\unknowncmd{x}"), "x");
        assert!(!latex_to_unicode(r"\vec{a} + \hat{b}").contains('\\'));
    }

    #[test]
    fn test_missing_object_state_returns_error() {
        let mut objects = HashMap::new();
        objects.insert(
            "c".into(),
            Object::Circle(CircleProps {
                cx: 50.0,
                cy: 50.0,
                radius: 20.0,
                z_index: 0,
                fill: "#FFF".into(),
                stroke: None,
                stroke_width: 0.0,
                shadow: None,
                opacity: 1.0,
            }),
        );
        // states is intentionally empty
        let states = HashMap::new();

        let mut r = make_renderer();
        let result = r.render_frame(&objects, &states, 100, 100, "#000000", None);
        assert!(
            result.is_err(),
            "render_frame should return Err when object has no state"
        );
    }
}

/// `hash01` seeds the analytic particle simulation, so its range is load-bearing:
/// every particle's position, lifetime, and velocity is derived from it.
#[cfg(test)]
mod hash_range {
    use crate::raster::hash01;

    #[test]
    fn hash01_stays_within_the_half_open_unit_interval() {
        // Sweep the whole u32 domain rather than sampling: the failure was at
        // the top of the range, where `u32::MAX as f32` rounds up and the
        // quotient reaches exactly 1.0. Stepping by a large prime covers the
        // space including the extremes without 4 billion iterations.
        const STEP: u32 = 2_654_435_761; // 2^32 / phi
        let mut n: u32 = 0;
        for _ in 0..2_000_000 {
            let h = hash01(n);
            assert!(
                (0.0..1.0).contains(&h),
                "hash01({n}) = {h} is outside [0, 1)"
            );
            n = n.wrapping_add(STEP);
        }
        // The extremes explicitly.
        for n in [0u32, 1, u32::MAX - 1, u32::MAX] {
            let h = hash01(n);
            assert!((0.0..1.0).contains(&h), "hash01({n}) = {h}");
        }
    }

    #[test]
    fn hash01_is_deterministic() {
        for n in [0u32, 7, 12345, u32::MAX] {
            assert_eq!(hash01(n).to_bits(), hash01(n).to_bits());
        }
    }
}

/// The frame buffer is reused between renders. These assert that reuse cannot
/// leak anything from one frame into the next.
///
/// This is the correctness risk the optimisation creates, and it is the kind
/// that would not show up as a crash — it would show up as a faint ghost of a
/// previous frame in one corner of a video, months later.
#[cfg(test)]
mod frame_buffer_reuse {
    use crate::{skia_backend::SkiaRenderer, Renderer};
    use luminafx_schema::{CircleProps, Object, RectangleProps};
    use serde_json::json;
    use std::collections::HashMap;

    fn circle_scene() -> (HashMap<String, Object>, HashMap<String, serde_json::Value>) {
        let mut objects = HashMap::new();
        objects.insert(
            "c".to_string(),
            Object::Circle(CircleProps {
                cx: 50.0,
                cy: 50.0,
                radius: 30.0,
                z_index: 1,
                fill: "#FF0000".into(),
                stroke: None,
                stroke_width: 0.0,
                shadow: None,
                opacity: 1.0,
            }),
        );
        let mut states = HashMap::new();
        states.insert(
            "c".to_string(),
            json!({"cx":50.0,"cy":50.0,"radius":30.0,"fill":"#FF0000","opacity":1.0,"z_index":1}),
        );
        (objects, states)
    }

    fn rect_scene() -> (HashMap<String, Object>, HashMap<String, serde_json::Value>) {
        let mut objects = HashMap::new();
        objects.insert(
            "r".to_string(),
            Object::Rectangle(RectangleProps {
                x: 10.0,
                y: 10.0,
                width: 20.0,
                height: 20.0,
                rx: 0.0,
                ry: 0.0,
                z_index: 1,
                fill: "#00FF00".into(),
                stroke: None,
                stroke_width: 0.0,
                shadow: None,
                opacity: 1.0,
            }),
        );
        let mut states = HashMap::new();
        states.insert(
            "r".to_string(),
            json!({"x":10.0,"y":10.0,"width":20.0,"height":20.0,"fill":"#00FF00",
                   "opacity":1.0,"z_index":1,"rx":0.0,"ry":0.0}),
        );
        (objects, states)
    }

    #[test]
    fn a_reused_buffer_renders_the_same_as_a_fresh_one() {
        let (c_objects, c_states) = circle_scene();
        let (r_objects, r_states) = rect_scene();

        // A renderer that has drawn something else first.
        let mut reused = SkiaRenderer::new();
        let _ = reused
            .render_frame(&c_objects, &c_states, 100, 100, "#000000", None)
            .expect("first render");
        let after_reuse = reused
            .render_frame(&r_objects, &r_states, 100, 100, "#000000", None)
            .expect("second render");

        // A renderer that has drawn nothing.
        let mut fresh = SkiaRenderer::new();
        let from_fresh = fresh
            .render_frame(&r_objects, &r_states, 100, 100, "#000000", None)
            .expect("fresh render");

        assert_eq!(
            after_reuse, from_fresh,
            "a reused buffer must produce byte-identical output to a fresh one; \
             anything else is a previous frame bleeding through"
        );
    }

    #[test]
    fn a_large_frame_followed_by_a_small_one_leaves_nothing_behind() {
        // The buffer is only reused when the dimensions match. Shrinking must
        // reallocate rather than render into a corner of the larger buffer.
        let (objects, states) = circle_scene();

        let mut reused = SkiaRenderer::new();
        let _ = reused
            .render_frame(&objects, &states, 200, 200, "#123456", None)
            .expect("large render");
        let small = reused
            .render_frame(&objects, &states, 60, 60, "#123456", None)
            .expect("small render");

        let mut fresh = SkiaRenderer::new();
        let expected = fresh
            .render_frame(&objects, &states, 60, 60, "#123456", None)
            .expect("fresh small render");

        assert_eq!(
            small.len(),
            60 * 60 * 4,
            "wrong buffer size after shrinking"
        );
        assert_eq!(
            small, expected,
            "shrinking must not reuse the larger buffer"
        );
    }

    #[test]
    fn repeated_renders_stay_identical() {
        // Determinism is the guarantee the whole engine rests on, and buffer
        // reuse is exactly the kind of change that could quietly break it.
        let (objects, states) = circle_scene();
        let mut r = SkiaRenderer::new();
        let first = r
            .render_frame(&objects, &states, 120, 90, "#0F0F1A", None)
            .expect("render");
        for i in 0..8 {
            let again = r
                .render_frame(&objects, &states, 120, 90, "#0F0F1A", None)
                .expect("render");
            assert_eq!(first, again, "render {i} differed from the first");
        }
    }

    #[test]
    fn a_failed_render_does_not_lose_the_buffer() {
        // An error path that dropped the buffer would make the next frame pay
        // the allocation this exists to avoid — a silent performance
        // regression with no test to catch it.
        let mut objects = HashMap::new();
        objects.insert(
            "bad".to_string(),
            Object::Arrow(luminafx_schema::ArrowProps {
                from: [0.0, 0.0],
                to: [1.0, 1.0],
                z_index: 1,
                color: "#FFFFFF".into(),
                stroke_width: 1.0,
                opacity: 1.0,
                label: None,
            }),
        );
        let mut states = HashMap::new();
        // A malformed `from` — the backends agree this is an error (#53).
        states.insert("bad".to_string(), json!({"from":[0.0],"to":[1.0,1.0]}));

        let mut r = SkiaRenderer::new();
        assert!(r
            .render_frame(&objects, &states, 64, 64, "#000000", None)
            .is_err());

        // The next render must still succeed and be correct.
        let (c_objects, c_states) = circle_scene();
        let after = r
            .render_frame(&c_objects, &c_states, 64, 64, "#000000", None)
            .expect("render after an error");
        let mut fresh = SkiaRenderer::new();
        let expected = fresh
            .render_frame(&c_objects, &c_states, 64, 64, "#000000", None)
            .expect("fresh render");
        assert_eq!(after, expected);
    }
}

/// The renderer must stay movable between threads.
///
/// Nothing in the renderer needs threading today, so nothing would fail if it
/// stopped being `Send` — until somebody tries to pipeline an export and finds
/// the type has been un-sendable for months, with no commit obviously to
/// blame. A caching change during Wave 3 did exactly this by reaching for `Rc`
/// where `Arc` was needed, and it compiled and passed every test.
#[cfg(test)]
mod thread_safety {
    use crate::skia_backend::SkiaRenderer;

    fn assert_send<T: Send>() {}

    #[test]
    fn the_cpu_renderer_is_send() {
        assert_send::<SkiaRenderer>();
    }

    #[test]
    fn the_text_engine_is_send() {
        assert_send::<luminafx_text::TextEngine>();
    }
}
