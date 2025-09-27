// GPUI-based connector rendering for diff viewer
// Adapted from the perfect implementation's connector rendering

use crate::advanced_diff::{ChangeType, MappingSegment};
use gpui::{
    AnyElement, Background, Bounds, Hsla, IntoElement, PaintQuad, ParentElement, PathBuilder,
    Pixels, Point, Styled, Window, canvas, div, hsla, point, px, transparent_black,
};

#[derive(Debug, Clone)]
pub struct ConnectorPoint {
    pub x: f32,
    pub y: f32,
    pub block_id: usize,
    pub is_start: bool,
}

#[derive(Debug, Clone)]
pub struct BezierConnector {
    pub id: usize,
    pub start_point: ConnectorPoint,
    pub end_point: ConnectorPoint,
    pub change_type: ChangeType,
    pub color: Hsla,
    pub is_hovered: bool,
}

impl BezierConnector {
    pub fn new(
        id: usize,
        start_point: ConnectorPoint,
        end_point: ConnectorPoint,
        change_type: ChangeType,
    ) -> Self {
        let color = match change_type {
            ChangeType::Addition => hsla(120.0 / 360.0, 0.6, 0.4, 0.3),
            ChangeType::Deletion => hsla(0.0 / 360.0, 0.6, 0.4, 0.3),
            ChangeType::Modification => hsla(210.0 / 360.0, 0.6, 0.4, 0.3),
            ChangeType::Context => hsla(0.0, 0.0, 0.5, 0.1),
        };

        Self {
            id,
            start_point,
            end_point,
            change_type,
            color,
            is_hovered: false,
        }
    }

    pub fn with_hover_state(mut self, is_hovered: bool) -> Self {
        self.is_hovered = is_hovered;
        if is_hovered {
            // Increase alpha for hover effect
            self.color = hsla(
                self.color.h,
                self.color.s,
                self.color.l,
                (self.color.a + 0.2).min(1.0),
            );
        }
        self
    }
}

pub struct ConnectorRenderer;

impl ConnectorRenderer {
    pub fn render_connectors(
        connectors: Vec<BezierConnector>,
        width: Pixels,
        height: Pixels,
    ) -> impl IntoElement {
        canvas(
            move |_bounds, _window, _cx| {},
            move |bounds, _data, window, _cx| {
                for connector in &connectors {
                    Self::render_single_connector(&connector, bounds, window);
                }
            },
        )
        .w(width)
        .h(height)
    }

    fn render_single_connector(
        connector: &BezierConnector,
        bounds: Bounds<Pixels>,
        window: &mut Window,
    ) {
        let start_x = bounds.origin.x + px(connector.start_point.x);
        let start_y = bounds.origin.y + px(connector.start_point.y);
        let end_x = bounds.origin.x + px(connector.end_point.x);
        let end_y = bounds.origin.y + px(connector.end_point.y);

        // Calculate control points for smooth bezier curve
        let control_distance = (end_x - start_x).abs() * 0.5;
        let control1_x = start_x + control_distance;
        let control1_y = start_y;
        let control2_x = end_x - control_distance;
        let control2_y = end_y;

        // Create bezier path using PathBuilder
        let mut path_builder = PathBuilder::stroke(px(1.5));
        path_builder.move_to(point(start_x, start_y));
        path_builder.cubic_bezier_to(
            point(end_x, end_y),
            point(control1_x, control1_y),
            point(control2_x, control2_y),
        );

        if let Ok(path) = path_builder.build() {
            window.paint_path(path, connector.color);
        }

        // Render filled area between connector lines
        Self::render_filled_area(connector, bounds, window);
    }

    fn render_filled_area(
        connector: &BezierConnector,
        bounds: Bounds<Pixels>,
        window: &mut Window,
    ) {
        // Create a filled quad for the connector area
        let start_x = bounds.origin.x + px(connector.start_point.x);
        let start_y = bounds.origin.y + px(connector.start_point.y);
        let end_x = bounds.origin.x + px(connector.end_point.x);
        let end_y = bounds.origin.y + px(connector.end_point.y);

        // Calculate height based on the change block size
        let height = px(20.0); // Standard line height

        let quad_bounds = Bounds {
            origin: Point {
                x: start_x,
                y: start_y,
            },
            size: gpui::Size {
                width: end_x - start_x,
                height: height,
            },
        };

        let background_color = Hsla {
            h: connector.color.h,
            s: connector.color.s,
            l: connector.color.l,
            a: connector.color.a * 0.3, // More transparent for background fill
        };

        window.paint_quad(PaintQuad {
            bounds: quad_bounds,
            background: background_color.into(),
            border_widths: Default::default(),
            border_color: transparent_black(),
            corner_radii: gpui::Corners::all(px(2.0)),
            border_style: Default::default(),
        });
    }

    pub fn render_middle_gutter_with_connectors(
        connectors: Vec<BezierConnector>,
        gutter_width: Pixels,
        total_height: Pixels,
        background_color: Hsla,
    ) -> impl IntoElement {
        div()
            .w(gutter_width)
            .h(total_height)
            .bg(background_color)
            .relative()
            .child(Self::render_connectors(
                connectors,
                gutter_width,
                total_height,
            ))
    }

    pub fn calculate_connector_positions(
        left_line_bounds: &[gpui::Bounds<Pixels>],
        right_line_bounds: &[gpui::Bounds<Pixels>],
        mapping_segments: &[MappingSegment],
        gutter_left_edge: f32,
        gutter_right_edge: f32,
    ) -> Vec<BezierConnector> {
        let mut connectors = Vec::new();

        for (id, segment) in mapping_segments.iter().enumerate() {
            // Find corresponding line bounds
            let left_index = (segment.left_start / 20.0) as usize; // Assuming 20px line height
            let right_index = (segment.right_start / 20.0) as usize;

            if let (Some(left_bounds), Some(right_bounds)) = (
                left_line_bounds.get(left_index),
                right_line_bounds.get(right_index),
            ) {
                let start_point = ConnectorPoint {
                    x: gutter_left_edge,
                    y: (left_bounds.origin.y + left_bounds.size.height / 2.0).0,
                    block_id: id,
                    is_start: true,
                };

                let end_point = ConnectorPoint {
                    x: gutter_right_edge,
                    y: (right_bounds.origin.y + right_bounds.size.height / 2.0).0,
                    block_id: id,
                    is_start: false,
                };

                // Determine change type based on y-position differences
                let left_center_y = left_bounds.origin.y + left_bounds.size.height / 2.0;
                let right_center_y = right_bounds.origin.y + right_bounds.size.height / 2.0;
                let change_type = if (left_center_y - right_center_y).abs() > px(5.0) {
                    ChangeType::Modification
                } else {
                    ChangeType::Context
                };

                connectors.push(BezierConnector::new(
                    id,
                    start_point,
                    end_point,
                    change_type,
                ));
            }
        }

        connectors
    }
}
