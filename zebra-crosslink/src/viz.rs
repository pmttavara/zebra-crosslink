//! Internal vizualization

use macroquad::{
    color::{self, colors::*},
    input::*,
    math::Vec2,
    shapes, text, window,
};

fn draw_horizontal_line(y: f32, stroke_thickness: f32, col: color::Color) {
    shapes::draw_line(0.0, y, window::screen_width(), y, stroke_thickness, col);
}
fn draw_vertical_line(x: f32, stroke_thickness: f32, col: color::Color) {
    shapes::draw_line(x, 0.0, x, window::screen_height(), stroke_thickness, col);
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

        shapes::draw_rectangle(
            window::screen_width() / 2.0 - 60.0,
            100.0,
            120.0,
            60.0,
            GREEN,
        );

        shapes::draw_circle(mouse_pt.x, mouse_pt.y, 10., YELLOW);

        draw_horizontal_line(mouse_pt.y, 1.0, DARKGRAY);
        draw_vertical_line(mouse_pt.x, 1.0, DARKGRAY);

        text::draw_text("HELLO", 20.0, 20.0, 20.0, DARKGRAY);

        window::next_frame().await
    }
}
