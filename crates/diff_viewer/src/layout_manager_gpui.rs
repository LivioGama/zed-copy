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
    ) {
        // Use approximate positioning when DisplayLine data isn't available
        // This provides basic connector functionality that matches the original egui version
        let line_height = self.config.line_height;
        let gutter_width = available_bounds.size.width.0;

        // Process imara blocks to create connectors with approximate positions
        for imara_block in &imara_analysis.blocks {
            if !imara_block.left_range.is_empty() && !imara_block.right_range.is_empty() {
                // Calculate approximate Y positions based on line numbers
                let left_start = imara_block.left_range.start;
                let left_end = imara_block.left_range.end.saturating_sub(1);
                let right_start = imara_block.right_range.start;
                let right_end = imara_block.right_range.end.saturating_sub(1);

                // Convert line numbers to approximate pixel positions
                let left_y_start = available_bounds.origin.y + px(left_start as f32 * line_height);
                let left_y_end =
                    available_bounds.origin.y + px((left_end + 1) as f32 * line_height);
                let right_y_start =
                    available_bounds.origin.y + px(right_start as f32 * line_height);
                let right_y_end =
                    available_bounds.origin.y + px((right_end + 1) as f32 * line_height);

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
        // Create bezier curves connecting the blocks
        let control_distance = px(30.0);

        // Top curve
        let start_top = point(x1, left_y_start);
        let end_top = point(x2, right_y_start);
        let control1_top = point(x1 + control_distance, left_y_start);
        let control2_top = point(x2 - control_distance, right_y_start);

        // Bottom curve
        let start_bottom = point(x1, left_y_end);
        let end_bottom = point(x2, right_y_end);
        let control1_bottom = point(x1 + control_distance, left_y_end);
        let control2_bottom = point(x2 - control_distance, right_y_end);

        // Create path for the filled connector region using PathBuilder
        let mut path_builder = PathBuilder::fill();
        path_builder.move_to(start_top);

        // Top bezier curve
        path_builder.curve_to(end_top, control2_top);

        // Right side
        path_builder.line_to(end_bottom);

        // Bottom bezier curve (reverse direction)
        path_builder.curve_to(start_bottom, control1_bottom);

        // Left side (close the shape)
        path_builder.line_to(start_top);

        // Fill the connector region
        if let Ok(path) = path_builder.build() {
            window.paint_path(path, color);
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
}

impl Default for LayoutManager {
    fn default() -> Self {
        Self::with_default_config(JetBrainsTheme::dark_theme())
    }
}
