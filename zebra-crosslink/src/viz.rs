//! Internal vizualization

use std::thread::JoinHandle;

use macroquad::{
    camera::*,
    color::{self, colors::*},
    input::*,
    math::{vec2, Circle, Rect, Vec2},
    shapes,
    text::{self, TextDimensions},
    time, window,
};

/// Common GUI state that may need to be passed around
#[derive(Debug)]
struct VizCtx {
    // h: BlockHeight,
    screen_o: Vec2,
    screen_vel: Vec2,
    fix_screen_o: Vec2,
    mouse_press: Vec2, // for determining drag
    mouse_drag_d: Vec2,
    old_mouse_pt: Vec2,
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

/// Viz implementation root
async fn viz_main(
    tokio_root_thread_handle: JoinHandle<()>,
) -> Result<(), crate::service::TFLServiceError> {
    let mut ctx = VizCtx {
        fix_screen_o: Vec2::ZERO,
        screen_o: Vec2::ZERO,
        screen_vel: Vec2::ZERO,
        mouse_press: Vec2::ZERO,
        mouse_drag_d: Vec2::ZERO,
        old_mouse_pt: {
            let (x, y) = mouse_position();
            Vec2 { x, y }
        },
    };

    loop {
        if tokio_root_thread_handle.is_finished() {
            break Ok(());
        }

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

                // used for momentum after letting go
                ctx.screen_vel = mouse_pt - ctx.old_mouse_pt; // ALT: average of last few frames?
            }
            ctx.mouse_drag_d = Vec2::ZERO;
        }

        if is_mouse_button_down(MouseButton::Left) {
            ctx.mouse_drag_d = mouse_pt - ctx.mouse_press;
            ctx.screen_vel = mouse_pt - ctx.old_mouse_pt;
            window::clear_background(BLUE);
        } else {
            let (scroll_x, scroll_y) = mouse_wheel();
            ctx.screen_vel += vec2(0.3 * scroll_x, 0.3 * scroll_y);

            window::clear_background(RED);
            ctx.fix_screen_o -= ctx.screen_vel; // apply "momentum"
            ctx.screen_vel = ctx.screen_vel.lerp(Vec2::ZERO, 0.12); // apply friction
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
        let dbg_str = format!(
            "{:2.3} ms\n\
            target: {}, offset: {}, zoom: {}\n\
            screen offset: {:8.3}, drag: {:7.3}, vel: {:7.3}",
            time::get_frame_time() * 1000.0,
            world_camera.target,
            world_camera.offset,
            world_camera.zoom,
            ctx.screen_o,
            ctx.mouse_drag_d,
            ctx.screen_vel
        );
        draw_multiline_text(&dbg_str, Vec2 { x: 10.0, y: 20.0 }, 20.0, None, WHITE);

        if true {
            // draw mouse point's world location
            let pt = world_camera.screen_to_world(mouse_pt);
            let old_pt = world_camera.screen_to_world(ctx.old_mouse_pt);
            draw_multiline_text(
                &format!("{}\n{}\n{}", mouse_pt, pt, old_pt),
                mouse_pt + vec2(5., -5.),
                20.,
                None,
                WHITE,
            );
        }

        ctx.old_mouse_pt = mouse_pt;
        window::next_frame().await
    }
}

/// Sync vizualization entry point wrapper (has to take place on main thread as an OS requirement)
pub fn main(tokio_root_thread_handle: JoinHandle<()>) {
    let config = window::Conf {
        window_title: "Zcash blocks".to_owned(),
        fullscreen: false,
        ..Default::default()
    };
    macroquad::Window::from_config(config, async {
        if let Err(err) = viz_main(tokio_root_thread_handle).await {
            macroquad::logging::error!("Error: {:?}", err);
        }
    });
}
