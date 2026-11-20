use crate::git::graph::GraphRow;
use crate::theme::{gh_colors, hex_color, rail_color};
use gpui_kit::{
    Bounds, Edges, IntoElement, ParentElement, PathBuilder, Pixels, Point, Styled, canvas, div,
    fill, point, px, size,
};

pub const ROW_HEIGHT: f32 = 30.0;
pub const LANE_WIDTH: f32 = 16.0;
pub const GRAPH_LEFT_PAD: f32 = 10.0;

#[inline]
fn lane_x(origin_x: Pixels, col: usize) -> Pixels {
    origin_x + px(GRAPH_LEFT_PAD + (col as f32 + 0.5) * LANE_WIDTH)
}

/// Computes pixel width needed for a given number of graph lanes.
pub fn graph_column_width(max_lanes: usize) -> Pixels {
    let lanes = max_lanes.clamp(2, 12);
    px(GRAPH_LEFT_PAD * 2.0 + lanes as f32 * LANE_WIDTH)
}

/// Renders the GPU rail graph cell for a commit row.
pub fn render_graph_cell(row: GraphRow, width: Pixels) -> impl IntoElement {
    let row_h = px(ROW_HEIGHT);

    div().w(width).h(row_h).flex_shrink_0().child(
        canvas(
            move |_, _, _| {},
            move |bounds, _, window, _| {
                let origin = bounds.origin;
                let top_y = origin.y;
                let mid_y = origin.y + px(ROW_HEIGHT * 0.5);
                let bot_y = origin.y + px(ROW_HEIGHT);

                // 1. Pass-through straight rails
                for &(col, color_idx) in &row.pass_through {
                    let x = lane_x(origin.x, col);
                    paint_segment(
                        window,
                        point(x, top_y),
                        point(x, bot_y),
                        rail_color(color_idx),
                    );
                }

                // 2. Top incoming edges (from top edge to commit dot)
                for edge in &row.top_edges {
                    let from = point(lane_x(origin.x, edge.from_col), top_y);
                    let to = point(lane_x(origin.x, edge.to_col), mid_y);
                    let color = rail_color(edge.color);
                    if edge.from_col == edge.to_col {
                        paint_segment(window, from, to, color);
                    } else {
                        paint_curve(window, from, to, color);
                    }
                }

                // 3. Bottom outgoing edges (from commit dot to bottom edge)
                for edge in &row.bottom_edges {
                    let from = point(lane_x(origin.x, edge.from_col), mid_y);
                    let to = point(lane_x(origin.x, edge.to_col), bot_y);
                    let color = rail_color(edge.color);
                    if edge.from_col == edge.to_col {
                        paint_segment(window, from, to, color);
                    } else {
                        paint_curve(window, from, to, color);
                    }
                }

                // 4. Commit dot (with dark canvas cutout ring, hollow for merge, double halo for HEAD)
                let center = point(lane_x(origin.x, row.commit_col), mid_y);
                let dot_color = rail_color(row.commit_color);
                let bg = hex_color(gh_colors::CANVAS_DEFAULT);

                if row.is_head {
                    // Outer crisp white/blue halo for active HEAD
                    paint_circle(window, center, px(6.5), dot_color.opacity(0.28));
                }

                // Dark background cutout around dot so lines don't collide visually
                paint_circle(window, center, px(5.5), bg);

                if row.is_merge {
                    paint_ring(window, center, px(4.5), bg, dot_color, px(2.0));
                } else if row.is_head {
                    paint_ring(
                        window,
                        center,
                        px(4.8),
                        dot_color,
                        hex_color(gh_colors::FG),
                        px(1.5),
                    );
                } else {
                    paint_circle(window, center, px(4.2), dot_color);
                }
            },
        )
        .size_full(),
    )
}

/// Renders the special WIP / working changes node at the top of the graph connecting down to lane 0.
pub fn render_wip_cell(width: Pixels, dirty: bool, head_col: usize) -> impl IntoElement {
    let row_h = px(ROW_HEIGHT);

    div().w(width).h(row_h).flex_shrink_0().child(
        canvas(
            move |_, _, _| {},
            move |bounds, _, window, _| {
                let origin = bounds.origin;
                let mid_y = origin.y + px(ROW_HEIGHT * 0.5);
                let bot_y = origin.y + px(ROW_HEIGHT);
                let cx = lane_x(origin.x, head_col);

                // Dashed downward stub connecting WIP to HEAD
                let color = if dirty {
                    hex_color(gh_colors::ORANGE)
                } else {
                    hex_color(gh_colors::FG_SUBTLE)
                };
                let mut builder = PathBuilder::stroke(px(1.8)).dash_array(&[px(3.0), px(3.0)]);
                builder.move_to(point(cx, mid_y));
                builder.line_to(point(cx, bot_y));
                if let Ok(path) = builder.build() {
                    window.paint_path(path, color);
                }

                let center = point(cx, mid_y);
                let bg = hex_color(gh_colors::CANVAS_DEFAULT);
                paint_ring(window, center, px(5.0), bg, color, px(2.0));
            },
        )
        .size_full(),
    )
}

fn paint_segment(
    window: &mut gpui_kit::Window,
    from: Point<Pixels>,
    to: Point<Pixels>,
    color: gpui_kit::Hsla,
) {
    let mut builder = PathBuilder::stroke(px(2.0));
    builder.move_to(from);
    builder.line_to(to);
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

fn paint_curve(
    window: &mut gpui_kit::Window,
    from: Point<Pixels>,
    to: Point<Pixels>,
    color: gpui_kit::Hsla,
) {
    let dy = (to.y - from.y) * 0.65;
    let ctrl_a = point(from.x, from.y + dy);
    let ctrl_b = point(to.x, to.y - dy);

    let mut builder = PathBuilder::stroke(px(2.0));
    builder.move_to(from);
    // Note: gpui PathBuilder::cubic_bezier_to takes (to, control_a, control_b).
    builder.cubic_bezier_to(to, ctrl_a, ctrl_b);
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

fn paint_circle(
    window: &mut gpui_kit::Window,
    center: Point<Pixels>,
    radius: Pixels,
    color: gpui_kit::Hsla,
) {
    let d = radius * 2.0;
    let bounds = Bounds::new(point(center.x - radius, center.y - radius), size(d, d));
    window.paint_quad(fill(bounds, color).corner_radii(radius));
}

fn paint_ring(
    window: &mut gpui_kit::Window,
    center: Point<Pixels>,
    radius: Pixels,
    bg: gpui_kit::Hsla,
    border: gpui_kit::Hsla,
    border_width: Pixels,
) {
    let d = radius * 2.0;
    let bounds = Bounds::new(point(center.x - radius, center.y - radius), size(d, d));
    window.paint_quad(
        fill(bounds, bg)
            .corner_radii(radius)
            .border_widths(Edges::all(border_width))
            .border_color(border),
    );
}
