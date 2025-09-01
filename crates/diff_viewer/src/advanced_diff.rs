// Simplified advanced diff algorithms - fixed version
// Using proper similar crate API for compatibility

use anyhow::Result;
use similar::{ChangeTag, TextDiff};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeType {
    Addition,
    Deletion,
    Modification,
    Context,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineState {
    Unchanged,
    Added,
    Deleted,
    Modified { word_diffs: Vec<WordDiff> },
    Placeholder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WordDiff {
    Same(String),
    Added(String),
    Deleted(String),
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub content: String,
    pub state: LineState,
    pub line_number: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct ChangeBlock {
    pub change_type: ChangeType,
    pub start_line_old: Option<usize>,
    pub end_line_old: Option<usize>,
    pub start_line_new: Option<usize>,
    pub end_line_new: Option<usize>,
    pub lines: Vec<DiffLine>,
    pub mapping_segments: Vec<MappingSegment>,
}

#[derive(Debug, Clone)]
pub struct MappingSegment {
    pub left_start: f32,
    pub left_end: f32,
    pub right_start: f32,
    pub right_end: f32,
    pub slope: f32,
}

#[derive(Debug, Clone)]
pub struct DiffView {
    pub blocks: Vec<ChangeBlock>,
    pub old_line_count: usize,
    pub new_line_count: usize,
}

pub struct AdvancedDiffGenerator;

impl AdvancedDiffGenerator {
    pub fn generate_diff(old_text: &str, new_text: &str) -> Result<DiffView> {
        let diff = TextDiff::from_lines(old_text, new_text);
        let mut blocks = Vec::new();
        let mut old_line_idx = 0;
        let mut new_line_idx = 0;

        for change in diff.iter_all_changes() {
            let change_type = match change.tag() {
                ChangeTag::Insert => ChangeType::Addition,
                ChangeTag::Delete => ChangeType::Deletion,
                ChangeTag::Equal => ChangeType::Context,
            };

            let line_content = change.value().to_string();
            let line_number_old = if change.tag() != ChangeTag::Insert {
                old_line_idx += 1;
                Some(old_line_idx)
            } else {
                None
            };
            let line_number_new = if change.tag() != ChangeTag::Delete {
                new_line_idx += 1;
                Some(new_line_idx)
            } else {
                None
            };

            let line_state = match change.tag() {
                ChangeTag::Insert => LineState::Added,
                ChangeTag::Delete => LineState::Deleted,
                ChangeTag::Equal => LineState::Unchanged,
            };

            let diff_line = DiffLine {
                content: line_content,
                state: line_state,
                line_number: line_number_new.or(line_number_old),
            };

            // Create simple blocks for each change
            blocks.push(ChangeBlock {
                change_type,
                start_line_old: line_number_old,
                end_line_old: line_number_old,
                start_line_new: line_number_new,
                end_line_new: line_number_new,
                lines: vec![diff_line],
                mapping_segments: Vec::new(),
            });
        }

        Ok(DiffView {
            blocks,
            old_line_count: old_line_idx,
            new_line_count: new_line_idx,
        })
    }
}