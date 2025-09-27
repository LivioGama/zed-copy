// src/highlight_renderer_gpui.rs
// GPUI Highlight renderer for JetBrains-style highlights

use crate::theme::JetBrainsTheme;
use gpui::{Bounds, Context, Hsla, Window};

/// Highlight renderer for JetBrains-style highlights using GPUI
pub struct HighlightRenderer {
    theme: JetBrainsTheme,
}

impl HighlightRenderer {
    pub fn new(theme: JetBrainsTheme) -> Self {
        Self { theme }
    }

    // Draw highlights based on line type using GPUI
    pub fn draw_highlight(
        &self,
        window: &mut Window,
        cx: &mut Context<'_, ()>,
        bounds: Bounds<gpui::Pixels>,
        line_type: &crate::models::line::LineType,
    ) {
        // Use theme-based highlight color based on line type
        let highlight_color = match line_type {
            crate::models::line::LineType::Addition => {
                // Green background for additions
                self.theme.addition_background_hsla()
            }
            crate::models::line::LineType::Deletion => {
                // Gray/red background for deletions
                self.theme.deletion_background_hsla()
            }
            crate::models::line::LineType::Context => {
                // Use modification background for context lines with changes
                self.theme.modification_background_hsla()
            }
            crate::models::line::LineType::Modification => {
                // Blue background for modifications
                self.theme.modification_background_hsla()
            }
        };

        if highlight_color.a > 0.0 {
            // TODO: Implement quad painting with proper GPUI API
            // cx.paint_quad(gpui::fill(bounds, highlight_color));
        }
    }

    pub fn get_highlight_color(&self, line_type: &crate::models::line::LineType) -> Hsla {
        match line_type {
            crate::models::line::LineType::Addition => self.theme.addition_background_hsla(),
            crate::models::line::LineType::Deletion => self.theme.deletion_background_hsla(),
            crate::models::line::LineType::Context => self.theme.modification_background_hsla(),
            crate::models::line::LineType::Modification => {
                self.theme.modification_background_hsla()
            }
        }
    }
}
