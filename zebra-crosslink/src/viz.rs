//! Internal vizualization

use macroquad::{
    camera::*,
    color::{self, colors::*},
    input::*,
    math::{vec2, Circle, Rect, Vec2},
    shapes,
    text::{self, TextDimensions},
    window,
};

/// Common GUI state that may need to be passed around
#[derive(Debug)]
struct VizCtx {
    // h: BlockHeight,
    screen_o: Vec2,
    fix_screen_o: Vec2,
    mouse_press: Vec2, // for determining drag
    mouse_drag_d: Vec2,
}

fn draw_line(pt0: Vec2, pt1: Vec2, stroke_thickness: f32, col: color::Color) {
    shapes::draw_line(pt0.x, pt0.y, pt1.x, pt1.y, stroke_thickness, col)
}
fn draw_horizontal_line(y: f32, stroke_thickness: f32, col: color::Color) {
    shapes::draw_line(0., y, window::screen_width(), y, stroke_thickness, col);
}
fn draw_vertical_line(x: f32, stroke_thickness: f32, col: color::Color) {
    shapes::draw_line(x, 0., x, window::screen_height(), stroke_thickness, col);
}
fn draw_rect(rect: Rect, col: color::Color) {
    shapes::draw_rectangle(rect.x, rect.y, rect.w, rect.h, col)
}
fn draw_circle(circle: Circle, col: color::Color) {
    shapes::draw_circle(circle.x, circle.y, circle.r, col)
}
fn draw_text(text: &str, pt: Vec2, font_size: f32, col: color::Color) -> TextDimensions {
    text::draw_text(text, pt.x, pt.y, font_size, col)
}
fn draw_multiline_text(
    text: &str,
    pt: Vec2,
    font_size: f32,
    line_distance_factor: Option<f32>,
    col: color::Color,
) {
    text::draw_multiline_text(text, pt.x, pt.y, font_size, line_distance_factor, col)
}

/// Vizualization entry point (has to take place on main thread as an OS requirement)
#[macroquad::main("Zcash blocks")]
pub async fn main() -> Result<(), crate::service::TFLServiceError> {
    let mut ctx = VizCtx {
        fix_screen_o: Vec2::ZERO,
        screen_o: Vec2::ZERO,
        mouse_press: Vec2::ZERO,
        mouse_drag_d: Vec2::ZERO,
    };

    loop {
        // INPUT ////////////////////////////////////////
        let mouse_pt = {
            let (x, y) = mouse_position();
            Vec2 { x, y }
        };
        if is_mouse_button_pressed(MouseButton::Left) {
            ctx.mouse_press = mouse_pt;
        } else {
            if is_mouse_button_released(MouseButton::Left) {
                ctx.fix_screen_o -= ctx.mouse_drag_d; // follow drag preview
            }
            ctx.mouse_drag_d = Vec2::ZERO;
        }

        if is_mouse_button_down(MouseButton::Left) {
            ctx.mouse_drag_d = mouse_pt - ctx.mouse_press;
            window::clear_background(BLUE);
        } else {
            window::clear_background(RED);
        }

        if is_key_down(KeyCode::Escape) {
            ctx.mouse_drag_d = Vec2::ZERO;
            ctx.mouse_press = mouse_pt;
        }

        ctx.screen_o = ctx.fix_screen_o - ctx.mouse_drag_d; // preview drag

        // WORLD SPACE DRAWING ////////////////////////////////////////
        let world_camera = Camera2D {
            target: ctx.screen_o,
            zoom: vec2(
                1. / window::screen_width() * 2.,
                1. / window::screen_height() * 2.,
            ),
            ..Default::default()
        };
        set_camera(&world_camera); // NOTE: can use push/pop camera state if useful

        draw_rect(
            Rect {
                x: window::screen_width() / 2. - 60.,
                y: 100.,
                w: 120.,
                h: 60.,
            },
            GREEN,
        );

        // SCREEN SPACE UI ////////////////////////////////////////
        set_default_camera();

        draw_horizontal_line(mouse_pt.y, 1., DARKGRAY);
        draw_vertical_line(mouse_pt.x, 1., DARKGRAY);

        // VIZ DEBUG INFO ////////////////////
        if true {
            // draw mouse point's world location
            let pt = world_camera.screen_to_world(mouse_pt);
            draw_multiline_text(
                &format!("{}\n{}", mouse_pt, pt),
                mouse_pt + vec2(5.0, -5.),
                20.,
                None,
                WHITE,
            );
        }

        window::next_frame().await
    }
}
