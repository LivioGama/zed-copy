// Connector rendering logic for diff viewer using GPUI
use gpui::{Bounds, Context, Hsla, Path, Window, point, px};

use crate::models::line::LineType;
use crate::theme::JetBrainsTheme;

pub struct ConnectorRenderer {
    theme: JetBrainsTheme,
}

impl ConnectorRenderer {
    pub fn new(theme: JetBrainsTheme) -> Self {
        Self { theme }
    }

    pub fn draw_connection_lines(
        &self,
        _window: &mut Window,
        _cx: &mut Context<'_, ()>,
        _old_lines: &[crate::models::line::DisplayLine],
        _new_lines: &[crate::models::line::DisplayLine],
    ) {
        // DISABLED: All connector rendering is handled by LayoutManager.render_connectors()
        // This prevents duplicate/overlapping connector rendering
    }

    fn draw_filled_connector_region(
        &self,
        _window: &mut Window,
        _cx: &mut Context<'_, ()>,
        _curve: &crate::models::ui::ConnectorCurve,
        _block_height: f32,
        _left_block_height: f32,
        _right_block_height: f32,
        _left_rects: &[Bounds<gpui::Pixels>],
        _right_rects: &[Bounds<gpui::Pixels>],
        _left_start: usize,
        _left_end: usize,
        _right_start: usize,
        _right_end: usize,
    ) {
        // DISABLED: All connector rendering is handled by LayoutManager.render_connectors()
        // This prevents duplicate/overlapping connector rendering
    }

    pub fn draw_change_block_connection(
        &self,
        _window: &mut Window,
        _cx: &mut Context<'_, ()>,
        _change_type: &str,
        _left_start: usize,
        _left_end: usize,
        _right_start: usize,
        _right_end: usize,
        _left_rects: &[Bounds<gpui::Pixels>],
        _right_rects: &[Bounds<gpui::Pixels>],
    ) {
        // DISABLED: All connector rendering is handled by LayoutManager.render_connectors()
        // This prevents duplicate/overlapping connector rendering
    }

    /// JetBrains-style connector rendering for independent line arrays
    fn draw_jetbrains_connectors(
        &self,
        _window: &mut Window,
        _cx: &mut Context<'_, ()>,
        _old_lines: &[crate::models::line::DisplayLine],
        _new_lines: &[crate::models::line::DisplayLine],
    ) {
        // DISABLED: All connector rendering is handled by LayoutManager.render_connectors()
        // This prevents duplicate/overlapping connector rendering
    }

    fn draw_single_connector(
        &self,
        window: &mut Window,
        cx: &mut Context<'_, ()>,
        start_point: gpui::Point<gpui::Pixels>,
        end_point: gpui::Point<gpui::Pixels>,
        color: Hsla,
        is_multi_line: bool,
    ) {
        // Calculate control points for bezier curve
        let control_distance = px(40.0);
        let control1 = point(start_point.x + control_distance, start_point.y);
        let control2 = point(end_point.x - control_distance, end_point.y);

        let thickness = if is_multi_line { px(4.0) } else { px(2.0) };

        // Create bezier path
        let mut path = Path::new(start_point);
        path.curve_to(end_point, control2);

        // Draw the connector as a stroked path
        // TODO: Implement path painting with proper GPUI API
        // cx.paint_path(path, color);
    }

    /// Find change blocks of a specific type in a line array
    fn find_change_blocks(
        &self,
        lines: &[crate::models::line::DisplayLine],
        line_type: LineType,
    ) -> Vec<(usize, usize)> {
        let mut blocks = Vec::new();
        let mut i = 0;

        while i < lines.len() {
            if lines[i].line_type == line_type {
                let start = i;
                let mut end = i;

                // Find consecutive lines of the same change type
                while end + 1 < lines.len() && lines[end + 1].line_type == line_type {
                    end += 1;
                }

                blocks.push((start, end));
                i = end + 1;
            } else {
                i += 1;
            }
        }

        blocks
    }

    fn evaluate_cubic_bezier(
        &self,
        p0: gpui::Point<gpui::Pixels>,
        p1: gpui::Point<gpui::Pixels>,
        p2: gpui::Point<gpui::Pixels>,
        p3: gpui::Point<gpui::Pixels>,
        t: f32,
    ) -> gpui::Point<gpui::Pixels> {
        let u = 1.0 - t;
        let tt = t * t;
        let uu = u * u;
        let uuu = uu * u;
        let ttt = tt * t;

        let x = uuu * p0.x.0 + 3.0 * uu * t * p1.x.0 + 3.0 * u * tt * p2.x.0 + ttt * p3.x.0;
        let y = uuu * p0.y.0 + 3.0 * uu * t * p1.y.0 + 3.0 * u * tt * p2.y.0 + ttt * p3.y.0;

        point(px(x), px(y))
    }

    pub fn draw_connector_between_bounds(
        &self,
        window: &mut Window,
        cx: &mut Context<'_, ()>,
        left_bounds: Bounds<gpui::Pixels>,
        right_bounds: Bounds<gpui::Pixels>,
        color: Hsla,
    ) {
        // Calculate connection points
        let left_point = point(
            left_bounds.origin.x + left_bounds.size.width,
            left_bounds.origin.y + left_bounds.size.height / 2.0,
        );
        let right_point = point(
            right_bounds.origin.x,
            right_bounds.origin.y + right_bounds.size.height / 2.0,
        );

        // Draw the connector
        self.draw_single_connector(window, cx, left_point, right_point, color, false);
    }

    pub fn draw_block_connector(
        &self,
        window: &mut Window,
        cx: &mut Context<'_, ()>,
        left_start_bounds: Bounds<gpui::Pixels>,
        left_end_bounds: Bounds<gpui::Pixels>,
        right_start_bounds: Bounds<gpui::Pixels>,
        right_end_bounds: Bounds<gpui::Pixels>,
        color: Hsla,
    ) {
        // Create a filled region between the blocks
        let left_x = left_start_bounds.origin.x + left_start_bounds.size.width;
        let right_x = right_start_bounds.origin.x;
        let top_y = left_start_bounds.origin.y.min(right_start_bounds.origin.y);
        let bottom_y = (left_end_bounds.origin.y + left_end_bounds.size.height)
            .max(right_end_bounds.origin.y + right_end_bounds.size.height);

        // Draw filled region
        let region_bounds = Bounds::from_corners(point(left_x, top_y), point(right_x, bottom_y));

        // TODO: Implement quad painting with proper GPUI API
        // cx.paint_quad(gpui::fill(region_bounds, color));
    }
}
