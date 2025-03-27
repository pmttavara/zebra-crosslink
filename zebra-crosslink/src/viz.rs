//! Internal vizualization

use crate::*;
use macroquad::{
    camera::*,
    color::{self, colors::*},
    input::*,
    math::{vec2, Circle, Rect, Vec2},
    shapes::{self, draw_triangle},
    text::{self, TextDimensions, TextParams},
    time, window,
};
use std::sync::Arc;
use std::thread::JoinHandle;

struct VizState {
    hash_start_height: BlockHeight,
    hashes: Vec<BlockHash>,
}
#[derive(Clone)]
struct VizGlobals {
    state: std::sync::Arc<VizState>,
    // wanted_height_rng: (u32, u32),
    consumed: bool, // adds one-way syncing so service_viz_requests doesn't run too quickly
}
static VIZ_G: std::sync::Mutex<Option<VizGlobals>> = std::sync::Mutex::new(None);

/// Bridge between tokio & viz code
pub async fn service_viz_requests(tfl_handle: crate::TFLServiceHandle) {
    let call = tfl_handle.call;

    *VIZ_G.lock().unwrap() = Some(VizGlobals {
        state: std::sync::Arc::new(VizState {
            hashes: Vec::new(),
            hash_start_height: BlockHeight(0),
        }),
        // wanted_height_rng: (0, 0),
        consumed: true,
    });

    loop {
        let old_g = VIZ_G.lock().unwrap().as_ref().unwrap().clone();
        if !old_g.consumed {
            std::thread::sleep(std::time::Duration::from_micros(500));
            continue;
        }
        let mut new_g = old_g.clone();
        new_g.consumed = false;

        let (hash_start_height, hashes) = {
            use std::ops::Sub;
            use zebra_chain::block::HeightDiff as BlockHeightDiff;

            if let Ok(ReadStateResponse::Tip(Some((tip_height, _)))) =
                (call.read_state)(ReadStateRequest::Tip).await
            {
                let start_height = tip_height
                    .sub(BlockHeightDiff::from(zebra_state::MAX_BLOCK_REORG_HEIGHT))
                    .unwrap_or(BlockHeight(0));
                if let Ok(ReadStateResponse::BlockHeader { hash, .. }) =
                    (call.read_state)(ReadStateRequest::BlockHeader(start_height.into())).await
                {
                    let (hashes, _) = tfl_block_sequence(&call, hash, None, true, false).await;
                    (start_height, hashes)
                } else {
                    error!("Failed to read start hash");
                    (BlockHeight(0), Vec::new())
                }
            } else {
                error!("Failed to read tip");
                (BlockHeight(0), Vec::new())
            }
        };

        let new_state = VizState {
            hash_start_height,
            hashes,
        };

        new_g.state = Arc::new(new_state);
        *VIZ_G.lock().unwrap() = Some(new_g);
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct BCNode {
    height: BlockHeight,
    hash: BlockHash,
    // ...
}

#[derive(Copy, Clone, Debug)]
enum NodeKind {
    BCNode(BCNode),
}

type NodeRef = Option<usize>;
#[derive(Copy, Clone, Debug)]
struct Node {
    parent: NodeRef,

    pt: Vec2,
    rad: f32,

    data: NodeKind,
}

impl Node {
    fn circle(&self) -> Circle {
        Circle::new(self.pt.x, self.pt.y, self.rad)
    }
}

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
    nodes: Vec<Node>,
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

/// `align` {0..=1, 0..=1} determines which point on the text's bounding box will be placed at `pt`
/// - Bottom-left (normal text) is {0,0}
/// - Bottom-right (right-aligned) is {1,0}
/// - Centred in both dimensions is: {0.5, 0.5}
///
/// N.B. this is the based on the dimensions provided by the font, which don't always match up
/// with a minimal bounding box nor a visual proportional weighting.
fn get_text_align_pt_ex(text: &str, pt: Vec2, params: &TextParams, align: Vec2) -> Vec2 {
    let align = align.clamp(Vec2::ZERO, Vec2::ONE);
    let dims = text::measure_text(text, params.font, params.font_size, params.font_scale);
    pt + vec2(-align.x * dims.width, align.y * dims.height)
}

fn get_text_align_pt(text: &str, pt: Vec2, font_size: f32, align: Vec2) -> Vec2 {
    get_text_align_pt_ex(
        text,
        pt,
        &TextParams {
            font_size: font_size as u16,
            font_scale: 1.,
            ..Default::default()
        },
        align,
    )
}

fn draw_text_align_ex(text: &str, pt: Vec2, params: TextParams, align: Vec2) -> TextDimensions {
    let new_pt = get_text_align_pt_ex(text, pt, &params, align);
    // bounding boxes:
    // shapes::draw_rectangle_lines(pt.x,     pt.y-dims.height,     dims.width, dims.height, 2., BLACK);
    // shapes::draw_rectangle_lines(new_pt.x, new_pt.y-dims.height, dims.width, dims.height, 2., YELLOW);
    text::draw_text_ex(text, new_pt.x, new_pt.y, params)
}

/// see [`draw_text_align_ex`]
fn draw_text_align(
    text: &str,
    pt: Vec2,
    font_size: f32,
    col: color::Color,
    align: Vec2,
) -> TextDimensions {
    draw_text_align_ex(
        text,
        pt,
        TextParams {
            font_size: font_size as u16,
            font_scale: 1.0,
            color: col,
            ..Default::default()
        },
        align,
    )
}

/// Currently only offsets in y; TODO: offset in arbitrary dimensions
fn pt_on_circle_edge(c: Circle, pt: Vec2) -> Vec2 {
    if pt.y < c.y {
        vec2(c.x, c.y - c.r)
    } else {
        vec2(c.x, c.y + c.r)
    }
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
        nodes: Vec::with_capacity(512),
    };

    let nodes = &mut ctx.nodes;

    // wait for servicing thread to init
    while VIZ_G.lock().unwrap().is_none() {
        // TODO: more efficient spin
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    loop {
        // TFL DATA ////////////////////////////////////////
        if tokio_root_thread_handle.is_finished() {
            break Ok(());
        }

        let g = {
            let mut lock = VIZ_G.lock().unwrap();
            lock.as_mut().unwrap().consumed = true;
            lock.as_ref().unwrap().clone()
        };

        // Cache nodes
        // TODO: handle non-contiguous chunks
        let mut parent = if nodes.len() > 0 { Some(nodes.len()-1) } else { None };
        for (i, hash) in g.state.hashes.iter().enumerate() {
            let new_node = BCNode {
                height: BlockHeight(g.state.hash_start_height.0 + i as u32), hash:*hash
            };

            // TODO: extract impl Eq for Node
            if nodes.iter().find(|node| match node.data {
                NodeKind::BCNode(node) => { node == new_node }
            }).is_none() {
                nodes.push(Node{
                    // TODO: distance should be proportional to difficulty of newer block
                    parent,
                    pt : parent.map_or(Vec2::ZERO, |i| nodes[i].pt - vec2(0., 100.)),
                    // TODO: base rad on num transactions?
                    // could overlay internal circle/rings for shielded/transparent
                    rad: 10.,
                    data: NodeKind::BCNode(new_node),

                });
                parent = Some(nodes.len()-1);
            }
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

        for (i, node) in nodes.iter().enumerate() {
            let col = WHITE; // TODO: depend on finality

            // NOTE: grows *upwards*
            let new_circle = node.circle();
            if i > 0 {
                let old_circle = nodes[i-1].circle();
                // draw arrow
                let arrow_bgn_pt = pt_on_circle_edge(old_circle, new_circle.point());
                let arrow_end_pt = pt_on_circle_edge(new_circle, old_circle.point());
                let line_thick = 2.;
                let arrow_size = 9.;
                let arrow_col = GRAY;
                let line_bgn_pt = arrow_bgn_pt - vec2(0., arrow_size);
                draw_line(line_bgn_pt, arrow_end_pt, line_thick, arrow_col);
                draw_triangle(
                    arrow_bgn_pt,
                    line_bgn_pt + vec2(0.5 * arrow_size, 0.),
                    line_bgn_pt - vec2(0.5 * arrow_size, 0.),
                    arrow_col,
                );
            }
            draw_circle(new_circle, col);

            // TODO: handle properly with new node structure
            let unique_chars_n = block_hash_unique_chars_n(&g.state.hashes[..]);

            match node.data {
                NodeKind::BCNode(bc_node) => {
                    let hash_str = &bc_node.hash.to_string()[..];
                    let unique_hash_str = &hash_str[hash_str.len() - unique_chars_n..];
                    let remain_hash_str = &hash_str[..hash_str.len() - unique_chars_n];
                    let font_size = 20.;

                    // NOTE: we use the full hash string for determining y-alignment
                    // need a single alignment point for baseline, otherwise the difference in heights
                    // between strings will make the baselines mismatch.
                    // TODO: use TextDimensions.offset_y to ensure matching baselines...
                    let text_align_y =
                        get_text_align_pt(hash_str, vec2(0., new_circle.y), font_size, vec2(1., 0.4)).y;

                    let pt = vec2(new_circle.x - (new_circle.r + 10.), text_align_y);

                    let text_dims = draw_text_align(
                        &format!(
                            "{} - {}",
                            unique_hash_str,
                            g.state.hash_start_height.0 as usize + i
                        ),
                        pt,
                        font_size,
                        WHITE,
                        vec2(1., 0.),
                    );
                    draw_text_align(
                        remain_hash_str,
                        pt - vec2(text_dims.width, 0.),
                        font_size,
                        LIGHTGRAY,
                        vec2(1., 0.),
                    );
                }
             }
        }

        // SCREEN SPACE UI ////////////////////////////////////////
        set_default_camera();

        draw_horizontal_line(mouse_pt.y, 1., DARKGRAY);
        draw_vertical_line(mouse_pt.x, 1., DARKGRAY);

        // VIZ DEBUG INFO ////////////////////
        let dbg_str = format!(
            "{:2.3} ms\n\
            target: {}, offset: {}, zoom: {}\n\
            screen offset: {:8.3}, drag: {:7.3}, vel: {:7.3}\n\
            {} hashes",
            time::get_frame_time() * 1000.,
            world_camera.target,
            world_camera.offset,
            world_camera.zoom,
            ctx.screen_o,
            ctx.mouse_drag_d,
            ctx.screen_vel,
            g.state.hashes.len()
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
