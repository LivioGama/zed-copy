// Layout manager for diff viewer using GPUI
use crate::models::line::DisplayLine;
use crate::theme::JetBrainsTheme;
use gpui::{Bounds, Hsla, PathBuilder, Window, point, px, size};

#[derive(Clone)]
pub struct LayoutConfig {
    pub connector_column_width: f32,
    pub gutter_width: f32,
    pub line_height: f32,
    pub font_size: f32,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            connector_column_width: 45.0,
            gutter_width: 45.0,
            line_height: 20.0,
            font_size: 14.0,
        }
    }
}

#[derive(Clone)]
pub struct LayoutManager {
    config: LayoutConfig,
    theme: JetBrainsTheme,
}

impl LayoutManager {
    pub fn new(config: LayoutConfig, theme: JetBrainsTheme) -> Self {
        Self { config, theme }
    }

    pub fn with_default_config(theme: JetBrainsTheme) -> Self {
        Self::new(LayoutConfig::default(), theme)
    }

    pub fn render_layout(
        &self,
        window: &mut Window,
        old_lines: &[DisplayLine],
        new_lines: &[DisplayLine],
        pane_width: f32,
        imara_analysis: &crate::ImaraDiffAnalysis,
        available_bounds: Bounds<gpui::Pixels>,
        left_scroll_y: f32,
        right_scroll_y: f32,
        left_line_height: f32,
        right_line_height: f32,
        header_height: f32,
    ) {
        // Calculate layout dimensions
        let gutter_x_start = pane_width;
        let connector_width = self.config.connector_column_width;

        // Render connectors between panes
        self.render_connectors(
            window,
            old_lines,
            new_lines,
            pane_width,
            imara_analysis,
            available_bounds,
            left_scroll_y,
            right_scroll_y,
            left_line_height,
            right_line_height,
            header_height,
        );
    }

    pub fn render_connectors(
        &self,
        window: &mut Window,
        _old_lines: &[DisplayLine],
        _new_lines: &[DisplayLine],
        _pane_width: f32,
        imara_analysis: &crate::ImaraDiffAnalysis,
        available_bounds: Bounds<gpui::Pixels>,
        left_scroll_y: f32,
        right_scroll_y: f32,
        left_line_height: f32,
        right_line_height: f32,
        header_height: f32,
    ) {
        // Use actual line heights from editors for precise positioning
        let gutter_width = available_bounds.size.width.0;

        // Process imara blocks to create connectors with approximate positions
        for imara_block in &imara_analysis.blocks {
            if !imara_block.left_range.is_empty() && !imara_block.right_range.is_empty() {
                // Calculate approximate Y positions based on line numbers
                let left_start = imara_block.left_range.start;
                let left_end = imara_block.left_range.end.saturating_sub(1);
                let right_start = imara_block.right_range.start;
                let right_end = imara_block.right_range.end.saturating_sub(1);

                // Convert line numbers to pixel positions using actual line heights, accounting for scroll and header
                let left_y_start = available_bounds.origin.y
                    + px(header_height + (left_start as f32 - left_scroll_y) * left_line_height);
                let left_y_end = available_bounds.origin.y
                    + px(header_height + ((left_end + 1) as f32 - left_scroll_y) * left_line_height);
                let right_y_start = available_bounds.origin.y
                    + px(header_height + (right_start as f32 - right_scroll_y) * right_line_height);
                let right_y_end = available_bounds.origin.y
                    + px(header_height
                        + ((right_end + 1) as f32 - right_scroll_y) * right_line_height);

                // Connector X coordinates (left edge to right edge of gutter)
                let x1 = available_bounds.origin.x; // Left edge of gutter
                let x2 = available_bounds.origin.x + px(gutter_width); // Right edge of gutter

                // Determine color based on Imara block operation
                let color = match imara_block.operation {
                    crate::ImaraBlockOperation::Modify => {
                        // Blue for modifications
                        Hsla {
                            h: 0.6,
                            s: 0.8,
                            l: 0.5,
                            a: 0.6,
                        }
                    }
                    crate::ImaraBlockOperation::Insert => {
                        // Green for additions
                        Hsla {
                            h: 0.33,
                            s: 0.7,
                            l: 0.5,
                            a: 0.6,
                        }
                    }
                    crate::ImaraBlockOperation::Delete => {
                        // Red for deletions
                        Hsla {
                            h: 0.0,
                            s: 0.8,
                            l: 0.5,
                            a: 0.6,
                        }
                    }
                };

                // Draw connector using GPUI APIs
                self.draw_connector(
                    window,
                    x1,
                    left_y_start,
                    left_y_end,
                    x2,
                    right_y_start,
                    right_y_end,
                    color,
                );
            }
        }
    }

    fn draw_connector(
        &self,
        window: &mut Window,
        x1: gpui::Pixels,
        left_y_start: gpui::Pixels,
        left_y_end: gpui::Pixels,
        x2: gpui::Pixels,
        right_y_start: gpui::Pixels,
        right_y_end: gpui::Pixels,
        color: Hsla,
    ) {
        use gpui::{point, px};

        let segments = 32; // High resolution for perfectly smooth curves
        let mut top_points = Vec::with_capacity(segments + 1);
        let mut bottom_points = Vec::with_capacity(segments + 1);

        let control_point_offset = (x2 - x1) * 0.35; // Optimal S-curve control point distance

        // Generate high-resolution curve points for top and bottom boundaries
        for i in 0..=segments {
            let t = i as f32 / segments as f32;

            // Top curve: start to end points with precise control point positioning
            let top_start = point(x1, left_y_start);
            let top_end = point(x2, right_y_start);
            let top_ctrl1 = point(x1 + control_point_offset, left_y_start);
            let top_ctrl2 = point(x2 - control_point_offset, right_y_start);
            top_points.push(self.cubic_bezier(top_start, top_ctrl1, top_ctrl2, top_end, t));

            // Bottom curve: parallel calculation for connector band creation
            let bottom_start = point(x1, left_y_end);
            let bottom_end = point(x2, right_y_end);
            let bottom_ctrl1 = point(x1 + control_point_offset, left_y_end);
            let bottom_ctrl2 = point(x2 - control_point_offset, right_y_end);
            bottom_points.push(self.cubic_bezier(
                bottom_start,
                bottom_ctrl1,
                bottom_ctrl2,
                bottom_end,
                t,
            ));
        }

        // Create filled shape using PathBuilder with line segments approximating the bezier curves
        let mut path_builder = gpui::PathBuilder::fill();
        if !top_points.is_empty() {
            path_builder.move_to(top_points[0]);

            // Add top curve points
            for point in top_points.iter().skip(1) {
                path_builder.line_to(*point);
            }

            // Add bottom curve points in reverse (to close the shape)
            for point in bottom_points.iter().rev() {
                path_builder.line_to(*point);
            }

            // Close the shape
            path_builder.line_to(top_points[0]);

            // Fill the connector region
            if let Ok(path) = path_builder.build() {
                window.paint_path(path, color);
            }
        }
    }

    pub fn calculate_line_bounds(
        &self,
        line_index: usize,
        x_offset: gpui::Pixels,
        y_offset: gpui::Pixels,
        line_width: gpui::Pixels,
    ) -> Bounds<gpui::Pixels> {
        let line_height = px(self.config.line_height);
        let y = y_offset + px(line_index as f32 * self.config.line_height);

        Bounds::new(point(x_offset, y), size(line_width, line_height))
    }

    pub fn get_connector_column_width(&self) -> f32 {
        self.config.connector_column_width
    }

    pub fn get_gutter_width(&self) -> f32 {
        self.config.gutter_width
    }

    fn cubic_bezier(
        &self,
        p0: gpui::Point<gpui::Pixels>,
        p1: gpui::Point<gpui::Pixels>,
        p2: gpui::Point<gpui::Pixels>,
        p3: gpui::Point<gpui::Pixels>,
        t: f32,
    ) -> gpui::Point<gpui::Pixels> {
        let u = 1.0 - t;
        let u2 = u * u; // Pre-calculate u²
        let u3 = u2 * u; // Pre-calculate u³
        let t2 = t * t; // Pre-calculate t²
        let t3 = t2 * t; // Pre-calculate t³

        // Optimized cubic bezier using pre-calculated powers
        point(
            gpui::Pixels(u3 * p0.x.0 + 3.0 * u2 * t * p1.x.0 + 3.0 * u * t2 * p2.x.0 + t3 * p3.x.0),
            gpui::Pixels(u3 * p0.y.0 + 3.0 * u2 * t * p1.y.0 + 3.0 * u * t2 * p2.y.0 + t3 * p3.y.0),
        )
    }
}

impl Default for LayoutManager {
    fn default() -> Self {
        Self::with_default_config(JetBrainsTheme::dark_theme())
    }
}
