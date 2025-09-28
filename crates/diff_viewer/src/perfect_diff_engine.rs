// Perfect JetBrains diff engine adapted for GPUI
// Advanced diff algorithms using both similar and imara-diff libraries

use imara_diff::{Algorithm, Diff, InternedInput};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq)]
pub enum LineType {
    Context,
    Addition,
    Deletion,
    Modification,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HighlightType {
    Insert,
    Delete,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LineState {
    Unchanged,
    Added,
    Deleted,
    Modified { word_diffs: Vec<WordDiff> },
    Placeholder,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WordDiff {
    Same(String),
    Added(String),
    Deleted(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChangeType {
    Addition,
    Deletion,
    Modification,
    Context,
}

#[derive(Debug, Clone)]
pub struct DisplayLine {
    pub content: String,
    pub line_type: LineType,
    pub original_line_num: Option<usize>,
    pub word_highlights: Vec<(usize, usize, HighlightType)>,
    pub state: LineState,
}

impl DisplayLine {
    pub fn new(content: String, line_type: LineType) -> Self {
        let state = match line_type {
            LineType::Addition => LineState::Added,
            LineType::Deletion => LineState::Deleted,
            LineType::Modification => LineState::Modified { word_diffs: vec![] },
            LineType::Context => LineState::Unchanged,
        };

        Self {
            content,
            line_type,
            original_line_num: None,
            word_highlights: Vec::new(),
            state,
        }
    }

    pub fn with_line_number(mut self, line_num: usize) -> Self {
        self.original_line_num = Some(line_num);
        self
    }

    pub fn with_word_diffs(mut self, word_diffs: Vec<WordDiff>) -> Self {
        self.state = LineState::Modified { word_diffs };
        self
    }
}

#[derive(Debug, Clone)]
pub struct NewChangeBlock {
    pub change_type: ChangeType,
    pub left_start_idx: usize,
    pub left_len: usize,
    pub right_start_idx: usize,
    pub right_len: usize,
}

#[derive(Debug, Clone)]
pub struct Line {
    pub content: String,
    pub state: LineState,
}

#[derive(Debug, Clone)]
pub struct DiffView {
    pub left_pane: Vec<Line>,
    pub right_pane: Vec<Line>,
    pub change_blocks: Vec<NewChangeBlock>,
}

#[derive(Debug, Clone)]
pub struct MappingSegment {
    pub left_start: f32,
    pub left_end: f32,
    pub right_start: f32,
    pub right_end: f32,
    pub slope: f32,
    pub left_tangent: f32,
    pub right_tangent: f32,
}

impl MappingSegment {
    pub fn new(left_start: f32, left_end: f32, right_start: f32, right_end: f32) -> Self {
        let slope = if (left_end - left_start).abs() > f32::EPSILON {
            (right_end - right_start) / (left_end - left_start)
        } else {
            0.0
        };

        Self {
            left_start,
            left_end,
            right_start,
            right_end,
            slope,
            left_tangent: 0.0,
            right_tangent: 0.0,
        }
    }

    pub fn map_left_to_right(&self, left_y: f32) -> f32 {
        if left_y < self.left_start {
            self.right_start
        } else if left_y > self.left_end {
            self.right_end
        } else {
            self.right_start + self.slope * (left_y - self.left_start)
        }
    }

    pub fn map_right_to_left(&self, right_y: f32) -> f32 {
        if right_y < self.right_start {
            self.left_start
        } else if right_y > self.right_end {
            self.left_end
        } else if self.slope.abs() > f32::EPSILON {
            self.left_start + (right_y - self.right_start) / self.slope
        } else {
            self.left_start
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImaraBlockOperation {
    Insert,
    Delete,
    Modify,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImaraDiffBlock {
    pub left_range: Range<usize>,
    pub right_range: Range<usize>,
    pub operation: ImaraBlockOperation,
    pub semantic_similarity: Option<f32>,
}

impl ImaraDiffBlock {
    pub fn new(
        left_range: Range<usize>,
        right_range: Range<usize>,
        operation: ImaraBlockOperation,
    ) -> Self {
        Self {
            left_range,
            right_range,
            operation,
            semantic_similarity: None,
        }
    }

    pub fn with_similarity(mut self, similarity: f32) -> Self {
        self.semantic_similarity = Some(similarity);
        self
    }

    pub fn is_change(&self) -> bool {
        true
    }

    pub fn is_pure_insertion(&self) -> bool {
        matches!(self.operation, ImaraBlockOperation::Insert) && self.left_range.is_empty()
    }

    pub fn is_pure_deletion(&self) -> bool {
        matches!(self.operation, ImaraBlockOperation::Delete) && self.right_range.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct ImaraDiffAnalysis {
    pub blocks: Vec<ImaraDiffBlock>,
}

#[derive(Debug, Clone)]
pub struct ImaraConfig {
    pub algorithm: Algorithm,
}

impl Default for ImaraConfig {
    fn default() -> Self {
        Self {
            algorithm: Algorithm::Histogram,
        }
    }
}

fn calculate_semantic_similarity(old_lines: &[&str], new_lines: &[&str]) -> f32 {
    if old_lines.is_empty() || new_lines.is_empty() {
        return 0.0;
    }

    let old_text = old_lines.join("\n");
    let new_text = new_lines.join("\n");

    if old_text == new_text {
        return 100.0;
    }

    // Simple similarity calculation based on common characters
    let old_chars: std::collections::HashSet<char> = old_text.chars().collect();
    let new_chars: std::collections::HashSet<char> = new_text.chars().collect();

    let intersection = old_chars.intersection(&new_chars).count();
    let union = old_chars.union(&new_chars).count();

    if union == 0 {
        0.0
    } else {
        (intersection as f32 / union as f32) * 100.0
    }
}

pub fn compute_imara_diff(
    old_content: &str,
    new_content: &str,
    config: &ImaraConfig,
) -> ImaraDiffAnalysis {
    let old_lines: Vec<&str> = old_content.lines().collect();
    let new_lines: Vec<&str> = new_content.lines().collect();

    let input = InternedInput::new(old_content, new_content);
    let mut diff = Diff::compute(config.algorithm, &input);
    diff.postprocess_lines(&input);

    let mut blocks = Vec::new();

    // Process hunks to build blocks and line mappings
    for hunk in diff.hunks() {
        let old_range = hunk.before.start as usize..hunk.before.end as usize;
        let new_range = hunk.after.start as usize..hunk.after.end as usize;

        let operation = if old_range.is_empty() {
            ImaraBlockOperation::Insert
        } else if new_range.is_empty() {
            ImaraBlockOperation::Delete
        } else {
            ImaraBlockOperation::Modify
        };

        // Calculate semantic similarity for the hunk
        let old_hunk_lines: Vec<&str> = if old_range.is_empty() {
            Vec::new()
        } else {
            old_lines[old_range.clone()].to_vec()
        };

        let new_hunk_lines: Vec<&str> = if new_range.is_empty() {
            Vec::new()
        } else {
            new_lines[new_range.clone()].to_vec()
        };

        let similarity = calculate_semantic_similarity(&old_hunk_lines, &new_hunk_lines);

        let block = ImaraDiffBlock::new(old_range.clone(), new_range.clone(), operation)
            .with_similarity(similarity);

        blocks.push(block);
    }

    ImaraDiffAnalysis { blocks }
}

pub fn compute_imara_diff_default(old_content: &str, new_content: &str) -> ImaraDiffAnalysis {
    compute_imara_diff(old_content, new_content, &ImaraConfig::default())
}

/// Perfect diff generator using Imara diff analysis for accurate line alignment
pub fn generate_perfect_diff_view(old_text: &str, new_text: &str) -> DiffView {
    let imara_analysis = compute_imara_diff_default(old_text, new_text);
    let old_lines: Vec<&str> = old_text.lines().collect();
    let new_lines: Vec<&str> = new_text.lines().collect();

    let mut left_pane = Vec::new();
    let mut right_pane = Vec::new();
    let mut change_blocks = Vec::new();

    let mut old_idx = 0;
    let mut new_idx = 0;

    for block in &imara_analysis.blocks {
        // Add unchanged lines before this block
        while old_idx < block.left_range.start && new_idx < block.right_range.start {
            if old_idx < old_lines.len() && new_idx < new_lines.len() {
                left_pane.push(Line {
                    content: old_lines[old_idx].to_string(),
                    state: LineState::Unchanged,
                });
                right_pane.push(Line {
                    content: new_lines[new_idx].to_string(),
                    state: LineState::Unchanged,
                });
                old_idx += 1;
                new_idx += 1;
            } else {
                break;
            }
        }

        // Handle the changed block
        let block_start_idx = left_pane.len();
        let left_len = block.left_range.len();
        let right_len = block.right_range.len();

        // Add changed lines from left side (deletions/modifications)
        for i in 0..left_len {
            let line_idx = block.left_range.start + i;
            if line_idx < old_lines.len() {
                let state = match block.operation {
                    ImaraBlockOperation::Delete => LineState::Deleted,
                    ImaraBlockOperation::Modify => {
                        // For modifications, we could add word diffs here
                        LineState::Modified { word_diffs: vec![] }
                    }
                    _ => LineState::Unchanged,
                };
                left_pane.push(Line {
                    content: old_lines[line_idx].to_string(),
                    state,
                });
            }
        }

        // Add changed lines from right side (insertions/modifications)
        for i in 0..right_len {
            let line_idx = block.right_range.start + i;
            if line_idx < new_lines.len() {
                let state = match block.operation {
                    ImaraBlockOperation::Insert => LineState::Added,
                    ImaraBlockOperation::Modify => {
                        // For modifications, we could add word diffs here
                        LineState::Modified { word_diffs: vec![] }
                    }
                    _ => LineState::Unchanged,
                };
                right_pane.push(Line {
                    content: new_lines[line_idx].to_string(),
                    state,
                });
            }
        }

        // Add placeholders to align the panes
        let max_len = left_len.max(right_len);
        if left_len < max_len {
            for _ in left_len..max_len {
                left_pane.push(Line {
                    content: "".to_string(),
                    state: LineState::Placeholder,
                });
            }
        }
        if right_len < max_len {
            for _ in right_len..max_len {
                right_pane.push(Line {
                    content: "".to_string(),
                    state: LineState::Placeholder,
                });
            }
        }

        // Create change block
        let change_type = match block.operation {
            ImaraBlockOperation::Insert => ChangeType::Addition,
            ImaraBlockOperation::Delete => ChangeType::Deletion,
            ImaraBlockOperation::Modify => ChangeType::Modification,
        };

        change_blocks.push(NewChangeBlock {
            change_type,
            left_start_idx: block_start_idx,
            left_len: max_len,
            right_start_idx: block_start_idx,
            right_len: max_len,
        });

        // Update indices
        old_idx = block.left_range.end;
        new_idx = block.right_range.end;
    }

    // Add remaining unchanged lines
    while old_idx < old_lines.len() && new_idx < new_lines.len() {
        left_pane.push(Line {
            content: old_lines[old_idx].to_string(),
            state: LineState::Unchanged,
        });
        right_pane.push(Line {
            content: new_lines[new_idx].to_string(),
            state: LineState::Unchanged,
        });
        old_idx += 1;
        new_idx += 1;
    }

    // Handle any remaining lines (shouldn't happen with proper diff, but safety check)
    while old_idx < old_lines.len() {
        left_pane.push(Line {
            content: old_lines[old_idx].to_string(),
            state: LineState::Unchanged,
        });
        right_pane.push(Line {
            content: "".to_string(),
            state: LineState::Placeholder,
        });
        old_idx += 1;
    }

    while new_idx < new_lines.len() {
        left_pane.push(Line {
            content: "".to_string(),
            state: LineState::Placeholder,
        });
        right_pane.push(Line {
            content: new_lines[new_idx].to_string(),
            state: LineState::Unchanged,
        });
        new_idx += 1;
    }

    DiffView {
        left_pane,
        right_pane,
        change_blocks,
    }
}
