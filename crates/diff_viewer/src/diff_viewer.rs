// Perfect JetBrains Diff Viewer - Clean implementation
// Only includes the new perfect algorithms and working GPUI integration

// Perfect implementation modules
pub mod perfect_diff_engine;
pub mod perfect_connector_renderer;

// Working simplified modules
mod advanced_diff;
mod connector_gpui;
mod diff_algorithm;

// Perfect implementation exports
pub use perfect_diff_engine::{
    generate_perfect_diff_view, compute_imara_diff_default, LineType, DisplayLine, 
    ImaraDiffAnalysis, ImaraDiffBlock, ImaraBlockOperation, DiffView as PerfectDiffView,
    Line, NewChangeBlock, MappingSegment as PerfectMappingSegment,
};
pub use perfect_connector_renderer::{PerfectConnectorRenderer, PerfectBezierConnector};

// Working simplified exports
pub use advanced_diff::{AdvancedDiffGenerator, DiffView, ChangeBlock, DiffLine, LineState, WordDiff, ChangeType, MappingSegment};
pub use connector_gpui::{ConnectorRenderer, BezierConnector as GpuiBezierConnector, ConnectorPoint};
pub use diff_algorithm::{DiffAlgorithm, Myers, Patience, DiffBlock, DiffBlockType};

// Legacy compatibility aliases
pub type JetBrainsDiffViewer = AdvancedDiffGenerator;
pub type JetBrainsEnhancedViewer = AdvancedDiffGenerator;