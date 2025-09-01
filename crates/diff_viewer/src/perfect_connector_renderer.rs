// Perfect JetBrains connector renderer adapted for GPUI
// Sophisticated bezier curve rendering with mesh-based approach

use gpui::{
    Bounds, Hsla, IntoElement, ParentElement, PathBuilder, Pixels, Point, Styled, Window,
    canvas, div, hsla, point, px, Corners, PaintQuad,
};
use crate::perfect_diff_engine::{LineType, MappingSegment, ImaraDiffAnalysis, ImaraBlockOperation};

#[derive(Debug, Clone)]
pub struct ConnectorPoint {
    pub x: f32,
    pub y: f32,
    pub block_id: usize,
    pub is_start: bool,
}

#[derive(Debug, Clone)]
pub struct PerfectBezierConnector {
    pub id: usize,
    pub start_point: ConnectorPoint,
    pub end_point: ConnectorPoint,
    pub line_type: LineType,
    pub color: Hsla,
    pub is_hovered: bool,
    pub thickness: f32,
}

impl PerfectBezierConnector {
    pub fn new(
        id: usize,
        start_point: ConnectorPoint,
        end_point: ConnectorPoint,
        line_type: LineType,
    ) -> Self {
        let (color, thickness) = match line_type {
            LineType::Addition => (hsla(120.0 / 360.0, 0.7, 0.5, 0.25), 2.0), // Green
            LineType::Deletion => (hsla(0.0 / 360.0, 0.7, 0.5, 0.25), 2.0),   // Red
            LineType::Modification => (hsla(240.0 / 360.0, 0.6, 0.5, 0.25), 3.0), // Blue, thicker
            LineType::Context => (hsla(200.0 / 360.0, 0.1, 0.7, 0.15), 1.0), // Light Gray
        };

        Self {
            id,
            start_point,
            end_point,
            line_type,
            color,
            is_hovered: false,
            thickness,
        }
    }

    pub fn with_hover_state(mut self, is_hovered: bool) -> Self {
        self.is_hovered = is_hovered;
        if is_hovered {
            self.color = Hsla {
                h: self.color.h,
                s: self.color.s,
                l: self.color.l,
                a: (self.color.a + 0.3).min(1.0),
            };
            self.thickness *= 1.5;
        }
        self
    }

    /// Render sophisticated connector band using cubic bezier curves
    pub fn render_connector_band(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        let start = point(px(self.start_point.x), px(self.start_point.y));
        let end = point(px(self.end_point.x), px(self.end_point.y));

        // Calculate control points for smooth bezier curve (JetBrains style)
        let control_distance = (end.x - start.x) * 0.35;
        let control1 = point(start.x + control_distance, start.y);
        let control2 = point(end.x - control_distance, end.y);

        // Create filled connector band using quad
        let height = px(self.thickness);
        let connector_bounds = Bounds {
            origin: Point { x: start.x, y: start.y - height / 2.0 },
            size: gpui::Size {
                width: end.x - start.x,
                height: height * 2.0,
            },
        };

        // Render filled background first
        window.paint_quad(PaintQuad {
            bounds: connector_bounds,
            background: self.color.into(),
            border_widths: Default::default(),
            border_color: gpui::transparent_black(),
            corner_radii: Corners::all(px(1.0)),
            border_style: Default::default(),
        });

        // Then render the bezier curve on top
        let mut path_builder = PathBuilder::stroke(px(self.thickness));
        path_builder.move_to(start);
        path_builder.cubic_bezier_to(end, control1, control2);

        if let Ok(path) = path_builder.build() {
            window.paint_path(path, self.color);
        }
    }
}

pub struct PerfectConnectorRenderer {
    pub connectors: Vec<PerfectBezierConnector>,
    pub scale_factor: f32,
    pub gutter_width: f32,
}

impl PerfectConnectorRenderer {
    pub fn new(scale_factor: f32, gutter_width: f32) -> Self {
        Self {
            connectors: Vec::new(),
            scale_factor,
            gutter_width,
        }
    }

    pub fn add_connector(&mut self, connector: PerfectBezierConnector) {
        self.connectors.push(connector);
    }

    pub fn clear_connectors(&mut self) {
        self.connectors.clear();
    }

    /// Generate connectors from Imara diff analysis (perfect mapping)
    pub fn generate_connectors_from_imara(
        &mut self,
        imara_analysis: &ImaraDiffAnalysis,
        line_height: f32,
        left_pane_width: f32,
        right_pane_x_offset: f32,
    ) {
        self.clear_connectors();
        let mut id_counter = 0;

        for imara_block in &imara_analysis.blocks {
            if !imara_block.is_change() {
                continue;
            }

            let line_type = match imara_block.operation {
                ImaraBlockOperation::Insert => LineType::Addition,
                ImaraBlockOperation::Delete => LineType::Deletion,
                ImaraBlockOperation::Modify => LineType::Modification,
            };

            // Calculate positions based on semantic ranges
            let left_start_y = imara_block.left_range.start as f32 * line_height;
            let left_end_y = imara_block.left_range.end as f32 * line_height;
            let right_start_y = imara_block.right_range.start as f32 * line_height;
            let right_end_y = imara_block.right_range.end as f32 * line_height;

            // Use center points for connection
            let left_center_y = (left_start_y + left_end_y) / 2.0;
            let right_center_y = (right_start_y + right_end_y) / 2.0;

            let start_point = ConnectorPoint {
                x: left_pane_width,
                y: left_center_y,
                block_id: id_counter,
                is_start: true,
            };

            let end_point = ConnectorPoint {
                x: right_pane_x_offset,
                y: right_center_y,
                block_id: id_counter,
                is_start: false,
            };

            self.add_connector(PerfectBezierConnector::new(
                id_counter,
                start_point,
                end_point,
                line_type,
            ));
            id_counter += 1;
        }
    }

    /// Generate connectors from mapping segments (legacy support)
    pub fn generate_connectors_from_mapping(
        &mut self,
        mapping_segments: &[MappingSegment],
        line_height: f32,
        left_pane_width: f32,
        right_pane_x_offset: f32,
    ) {
        self.clear_connectors();
        let mut id_counter = 0;

        for segment in mapping_segments {
            let start_point = ConnectorPoint {
                x: left_pane_width,
                y: (segment.left_start + segment.left_end) / 2.0 * line_height,
                block_id: id_counter,
                is_start: true,
            };

            let end_point = ConnectorPoint {
                x: right_pane_x_offset,
                y: (segment.right_start + segment.right_end) / 2.0 * line_height,
                block_id: id_counter,
                is_start: false,
            };

            self.add_connector(PerfectBezierConnector::new(
                id_counter,
                start_point,
                end_point,
                LineType::Modification, // Default to modification
            ));
            id_counter += 1;
        }
    }

    /// Render all connectors in the middle gutter
    pub fn render_middle_gutter(&self, total_height: Pixels) -> impl IntoElement {
        let connectors = self.connectors.clone();
        let gutter_width = px(self.gutter_width);
        let background_color = hsla(0.0, 0.0, 0.18, 1.0); // Dark gutter background

        div()
            .w(gutter_width)
            .h(total_height)
            .bg(background_color)
            .border_x_1()
            .border_color(hsla(0.0, 0.0, 0.24, 1.0))
            .relative()
            .child(
                canvas(
                    move |_bounds, _window, _cx| {},
                    move |bounds, _data, window, _cx| {
                        for connector in &connectors {
                            connector.render_connector_band(bounds, window);
                        }
                    }
                )
                .w_full()
                .h_full()
            )
    }
}

impl IntoElement for PerfectConnectorRenderer {
    type Element = gpui::AnyElement;

    fn into_element(self) -> Self::Element {
        self.render_middle_gutter(px(600.0)).into_any_element()
    }
}
