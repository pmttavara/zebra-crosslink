//! Internal vizualization

use macroquad::{
    color::{self, colors::*},
    input::*,
    math::{vec2, Circle, Rect, Vec2},
    shapes,
    text::{self, TextDimensions},
    window,
};

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

/// Vizualization entry point (has to take place on main thread as an OS requirement)
#[macroquad::main("Zcash blocks")]
pub async fn main() -> Result<(), crate::service::TFLServiceError> {
    loop {
        let mouse_pt = {
            let (x, y) = mouse_position();
            Vec2 { x, y }
        };

        if is_mouse_button_down(MouseButton::Left) {
            window::clear_background(BLUE);
        } else {
            window::clear_background(RED);
        }

        draw_rect(
            Rect {
                x: window::screen_width() / 2. - 60.,
                y: 100.,
                w: 120.,
                h: 60.,
            },
            GREEN,
        );

        draw_circle(
            Circle {
                x: mouse_pt.x,
                y: mouse_pt.y,
                r: 3.,
            },
            YELLOW,
        );

        draw_horizontal_line(mouse_pt.y, 1., DARKGRAY);
        draw_vertical_line(mouse_pt.x, 1., DARKGRAY);

        draw_text(
            &format!("{}", mouse_pt),
            mouse_pt + vec2(5., -5.),
            20.,
            WHITE,
        );

        window::next_frame().await
    }
}
