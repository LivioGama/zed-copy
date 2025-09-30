use gpui::Pixels;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use settings::{Settings, SettingsContent};

#[derive(Clone, Default, Serialize, Deserialize, JsonSchema, Debug)]
pub struct SplitDiffSettingsContent {
    /// Default view mode for split diff.
    ///
    /// Default: split
    pub default_view: Option<SplitDiffViewMode>,
    /// Number of context lines to show around changes.
    ///
    /// Default: 3
    pub context_lines: Option<u32>,
    /// How to handle whitespace in diffs.
    ///
    /// Default: none
    pub ignore_whitespace: Option<WhitespaceMode>,
    /// Whether to synchronize scrolling between left and right panes.
    ///
    /// Default: true
    pub sync_scroll: Option<bool>,
    /// How to highlight intra-line changes.
    ///
    /// Default: word
    pub intraline: Option<IntralineMode>,
    /// Whether to wrap long lines.
    ///
    /// Default: false
    pub word_wrap: Option<bool>,
    /// Default width of the split diff view in pixels.
    ///
    /// Default: 800
    pub default_width: Option<f32>,
    /// Default height of the split diff view in pixels.
    ///
    /// Default: 600
    pub default_height: Option<f32>,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SplitDiffViewMode {
    Split,
    Unified,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum WhitespaceMode {
    None,
    Eol,
    All,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum IntralineMode {
    Off,
    Word,
    Char,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SplitDiffSettings {
    pub default_view: SplitDiffViewMode,
    pub context_lines: u32,
    pub ignore_whitespace: WhitespaceMode,
    pub sync_scroll: bool,
    pub intraline: IntralineMode,
    pub word_wrap: bool,
    pub default_width: Pixels,
    pub default_height: Pixels,
}

impl Default for SplitDiffSettings {
    fn default() -> Self {
        Self {
            default_view: SplitDiffViewMode::Unified,
            context_lines: 3,
            ignore_whitespace: WhitespaceMode::None,
            sync_scroll: true,
            intraline: IntralineMode::Word,
            word_wrap: false,
            default_width: Pixels(800.0),
            default_height: Pixels(600.0),
        }
    }
}

impl Settings for SplitDiffSettings {
    fn from_settings(_content: &SettingsContent, _cx: &mut gpui::App) -> Self {
        Self::default()
    }

    fn import_from_vscode(_vscode: &settings::VsCodeSettings, _current: &mut SettingsContent) {}
}

// TODO: Implement SettingsUi once the API is stable
// impl settings::SettingsUi for SplitDiffSettings { ... }
