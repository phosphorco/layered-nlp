//! Document-level annotated text display (FR-010 Gate 3).
//!
//! This module provides `DocDisplay` which renders a multi-line annotated view
//! of a snapshot, showing spans as ASCII overlays on the original text.
//!
//! The output format extends the `LLLineDisplay` pattern from layered-nlp to
//! document-level snapshots.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::{self, Write};

use super::types::{InputSource, Snapshot, SpanData, SnapshotSpanId};

/// Configuration and state for rendering annotated document display.
pub struct DocDisplay<'a> {
    snapshot: &'a Snapshot,
    /// Types to include (empty = include all)
    include_types: HashSet<String>,
    /// Types to show with associations
    association_types: HashSet<String>,
    /// Whether to show line numbers
    show_line_numbers: bool,
    /// Verbose mode (always show confidence, etc.)
    verbose: bool,
}

impl<'a> DocDisplay<'a> {
    /// Create a new display for the given snapshot.
    pub fn new(snapshot: &'a Snapshot) -> Self {
        Self {
            snapshot,
            include_types: HashSet::new(),
            association_types: HashSet::new(),
            show_line_numbers: true,
            verbose: false,
        }
    }

    /// Include a specific type in the display.
    pub fn include(mut self, type_name: &str) -> Self {
        self.include_types.insert(type_name.to_string());
        self
    }

    /// Include a type and show its associations.
    pub fn include_with_associations(mut self, type_name: &str) -> Self {
        self.include_types.insert(type_name.to_string());
        self.association_types.insert(type_name.to_string());
        self
    }

    /// Hide line numbers.
    pub fn without_line_numbers(mut self) -> Self {
        self.show_line_numbers = false;
        self
    }

    /// Enable verbose mode.
    pub fn verbose(mut self) -> Self {
        self.verbose = true;
        self
    }

    /// Get the input lines from the snapshot.
    ///
    /// For `FileRef` snapshots, returns a placeholder message. Span overlays
    /// are not meaningful without the actual file content.
    fn get_lines(&self) -> Vec<&str> {
        match &self.snapshot.input {
            InputSource::Inline(lines) => lines.iter().map(|s| s.as_str()).collect(),
            InputSource::FileRef(_) => vec![
                "[file reference - annotated view unavailable; see .ron snapshot for data]"
            ],
        }
    }

    /// Check if a span should be included.
    fn should_include(&self, span: &SpanData) -> bool {
        if self.include_types.is_empty() {
            true
        } else {
            self.include_types.contains(&span.type_name)
        }
    }

    /// Build an index of line -> spans that start on that line.
    fn build_line_index(&self) -> BTreeMap<u32, Vec<&SpanData>> {
        let mut index: BTreeMap<u32, Vec<&SpanData>> = BTreeMap::new();
        
        for spans in self.snapshot.spans.values() {
            for span in spans {
                if self.should_include(span) {
                    index
                        .entry(span.position.start.line)
                        .or_default()
                        .push(span);
                }
            }
        }
        
        // Sort spans within each line by start token, then by end token (descending for outer-first)
        for spans in index.values_mut() {
            spans.sort_by(|a, b| {
                let cmp = a.position.start.token.cmp(&b.position.start.token);
                if cmp == std::cmp::Ordering::Equal {
                    // Longer spans first (so they're rendered on outer layer)
                    b.position.end.token.cmp(&a.position.end.token)
                } else {
                    cmp
                }
            });
        }
        
        index
    }

    /// Build a map of span IDs that are referenced by associations -> labels.
    ///
    /// Labels are only assigned to spans that:
    /// 1. Are targets of associations from spans with association display enabled
    /// 2. Are themselves included in the current display (via should_include)
    ///
    /// This prevents confusing labels like `[A]` that point to non-visible spans.
    fn build_span_labels(&self) -> HashMap<SnapshotSpanId, String> {
        let mut targets: HashSet<SnapshotSpanId> = HashSet::new();
        
        // Find all spans that are association targets
        for spans in self.snapshot.spans.values() {
            for span in spans {
                if self.association_types.contains(&span.type_name) {
                    for assoc in &span.associations {
                        targets.insert(assoc.target.clone());
                    }
                }
            }
        }
        
        // Filter to only targets whose spans are actually included in display
        let included_targets: HashSet<SnapshotSpanId> = targets
            .into_iter()
            .filter(|id| {
                self.snapshot.find_by_id(id)
                    .map(|span| self.should_include(span))
                    .unwrap_or(false)
            })
            .collect();
        
        // Assign labels in sorted order
        let mut sorted_targets: Vec<_> = included_targets.into_iter().collect();
        sorted_targets.sort();
        
        sorted_targets
            .into_iter()
            .enumerate()
            .map(|(i, id)| (id, index_to_label(i)))
            .collect()
    }

    /// Render a summary of a span value.
    ///
    /// Uses char-based truncation for Unicode safety.
    fn render_value_summary(&self, span: &SpanData) -> String {
        const MAX_LEN: usize = 30;
        const TRUNC_LEN: usize = 27;
        
        match &span.value {
            ron::Value::String(s) => {
                if s.chars().count() > MAX_LEN {
                    let truncated: String = s.chars().take(TRUNC_LEN).collect();
                    format!("{}...", truncated)
                } else {
                    s.clone()
                }
            }
            _ => format!("{:?}", span.value),
        }
    }
}

/// Convert index to label: 0 -> "A", 1 -> "B", ..., 25 -> "Z", 26 -> "AA", etc.
pub fn index_to_label(mut n: usize) -> String {
    let mut result = String::new();
    loop {
        let remainder = n % 26;
        result.insert(0, (b'A' + remainder as u8) as char);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    format!("[{}]", result)
}

impl<'a> fmt::Display for DocDisplay<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lines = self.get_lines();
        let line_index = self.build_line_index();
        let span_labels = self.build_span_labels();
        
        // Build index of multi-line spans that touch each line
        let multiline_index = self.build_multiline_index();
        
        // Calculate line number width for alignment
        let line_num_width = if self.show_line_numbers {
            if lines.is_empty() {
                1
            } else {
                (lines.len() as f64).log10().ceil().max(1.0) as usize
            }
        } else {
            0
        };
        
        for (line_idx, line_text) in lines.iter().enumerate() {
            let line_num = line_idx as u32;
            
            // Render line with line number
            if self.show_line_numbers {
                write!(f, "{:>width$}│ ", line_num + 1, width = line_num_width)?;
            }
            writeln!(f, "{}", line_text)?;
            
            // Render span annotations for this line
            if let Some(spans) = line_index.get(&line_num) {
                for span in spans {
                    if span.position.start.line == span.position.end.line {
                        // Single-line span
                        self.render_single_line_span(f, span, line_text, line_num_width, &span_labels)?;
                    } else {
                        // Multi-line span start
                        self.render_multiline_span_start(f, span, line_text, line_num_width, &span_labels)?;
                    }
                }
            }
            
            // Render multi-line span continuations and endings
            if let Some(spans) = multiline_index.get(&line_num) {
                for span in spans {
                    let is_start = span.position.start.line == line_num;
                    let is_end = span.position.end.line == line_num;
                    
                    if is_end && !is_start {
                        // This is the end line of a multi-line span
                        self.render_multiline_span_end(f, span, line_text, line_num_width, &span_labels)?;
                    } else if !is_start && !is_end {
                        // This is a middle line of a multi-line span
                        self.render_multiline_span_middle(f, span, line_text, line_num_width)?;
                    }
                    // Note: start line is already handled by line_index
                }
            }
        }
        
        Ok(())
    }
}

impl<'a> DocDisplay<'a> {
    /// Render a single-line span annotation.
    fn render_single_line_span(
        &self,
        f: &mut fmt::Formatter<'_>,
        span: &SpanData,
        line_text: &str,
        line_num_width: usize,
        span_labels: &HashMap<SnapshotSpanId, String>,
    ) -> fmt::Result {
        // Calculate approximate character position from token position
        // This is a simplification; in reality we'd need token positions from the document
        let start_pos = estimate_char_pos(line_text, span.position.start.token as usize);
        let end_pos = estimate_char_pos(line_text, span.position.end.token as usize + 1);
        
        // Indent for line number column
        if self.show_line_numbers {
            write!(f, "{:width$}  ", "", width = line_num_width)?;
        }
        
        // Spaces to start position
        for _ in 0..start_pos {
            f.write_char(' ')?;
        }
        
        // Draw the underline
        f.write_char('╰')?;
        let span_width = end_pos.saturating_sub(start_pos);
        for _ in 1..span_width.saturating_sub(1) {
            f.write_char('─')?;
        }
        if span_width > 1 {
            f.write_char('╯')?;
        }
        
        // Add label if this span is a target
        if let Some(label) = span_labels.get(&span.id) {
            write!(f, "{} ", label)?;
        }
        
        // Add span ID and type summary
        write!(f, "[{}] ", span.id)?;
        
        // Add value summary
        let summary = self.render_value_summary(span);
        write!(f, "{}", summary)?;
        
        // Add confidence if verbose or low
        if let Some(conf) = span.confidence {
            if self.verbose || conf < 0.8 {
                write!(f, " ({:.2})", conf)?;
            }
        }
        
        writeln!(f)?;
        
        // Render associations if enabled
        if self.association_types.contains(&span.type_name) {
            for assoc in &span.associations {
                // Indent for line number + offset
                if self.show_line_numbers {
                    write!(f, "{:width$}  ", "", width = line_num_width)?;
                }
                for _ in 0..start_pos + 2 {
                    f.write_char(' ')?;
                }
                
                let glyph = assoc.glyph.as_deref().unwrap_or("");
                let target_label = span_labels.get(&assoc.target)
                    .map(|s| s.as_str())
                    .unwrap_or(&assoc.target.0);
                
                writeln!(f, "└─{}{}→{}", glyph, assoc.label, target_label)?;
            }
        }
        
        Ok(())
    }
    
    /// Build an index of line -> multi-line spans that touch that line (not just start).
    fn build_multiline_index(&self) -> BTreeMap<u32, Vec<&SpanData>> {
        let mut index: BTreeMap<u32, Vec<&SpanData>> = BTreeMap::new();
        
        for spans in self.snapshot.spans.values() {
            for span in spans {
                if self.should_include(span) && span.position.start.line != span.position.end.line {
                    // This is a multi-line span; add it to all lines it touches
                    for line in span.position.start.line..=span.position.end.line {
                        index.entry(line).or_default().push(span);
                    }
                }
            }
        }
        
        index
    }
    
    /// Render the start of a multi-line span.
    fn render_multiline_span_start(
        &self,
        f: &mut fmt::Formatter<'_>,
        span: &SpanData,
        line_text: &str,
        line_num_width: usize,
        span_labels: &HashMap<SnapshotSpanId, String>,
    ) -> fmt::Result {
        let start_pos = estimate_char_pos(line_text, span.position.start.token as usize);
        
        // Indent for line number column
        if self.show_line_numbers {
            write!(f, "{:width$}  ", "", width = line_num_width)?;
        }
        
        // Spaces to start position
        for _ in 0..start_pos {
            f.write_char(' ')?;
        }
        
        // Start marker for multi-line span
        f.write_char('╭')?;
        
        // Fill to end of line
        let remaining = line_text.len().saturating_sub(start_pos + 1);
        for _ in 0..remaining {
            f.write_char('─')?;
        }
        
        // Add label if this span is a target
        if let Some(label) = span_labels.get(&span.id) {
            write!(f, " {}", label)?;
        }
        
        // Add span ID
        write!(f, " [{}]", span.id)?;
        
        writeln!(f)?;
        Ok(())
    }
    
    /// Render a middle line of a multi-line span (continuation marker).
    fn render_multiline_span_middle(
        &self,
        f: &mut fmt::Formatter<'_>,
        _span: &SpanData,
        _line_text: &str,
        line_num_width: usize,
    ) -> fmt::Result {
        // Indent for line number column
        if self.show_line_numbers {
            write!(f, "{:width$}  ", "", width = line_num_width)?;
        }
        
        // The continuation marker is placed at the start position (column 0 for simplicity)
        // A more accurate implementation would use span.position.start.token
        f.write_char('│')?;
        writeln!(f)?;
        Ok(())
    }
    
    /// Render the end of a multi-line span.
    fn render_multiline_span_end(
        &self,
        f: &mut fmt::Formatter<'_>,
        span: &SpanData,
        line_text: &str,
        line_num_width: usize,
        span_labels: &HashMap<SnapshotSpanId, String>,
    ) -> fmt::Result {
        let end_pos = estimate_char_pos(line_text, span.position.end.token as usize + 1);
        
        // Indent for line number column
        if self.show_line_numbers {
            write!(f, "{:width$}  ", "", width = line_num_width)?;
        }
        
        // Draw the closing bracket
        f.write_char('╰')?;
        for _ in 1..end_pos.saturating_sub(1) {
            f.write_char('─')?;
        }
        if end_pos > 1 {
            f.write_char('╯')?;
        }
        
        // Add span ID and value summary
        write!(f, " [{}] ", span.id)?;
        let summary = self.render_value_summary(span);
        write!(f, "{}", summary)?;
        
        // Add confidence if verbose or low
        if let Some(conf) = span.confidence {
            if self.verbose || conf < 0.8 {
                write!(f, " ({:.2})", conf)?;
            }
        }
        
        writeln!(f)?;
        
        // Render associations if enabled
        if self.association_types.contains(&span.type_name) {
            for assoc in &span.associations {
                if self.show_line_numbers {
                    write!(f, "{:width$}  ", "", width = line_num_width)?;
                }
                for _ in 0..2 {
                    f.write_char(' ')?;
                }
                
                let glyph = assoc.glyph.as_deref().unwrap_or("");
                let target_label = span_labels.get(&assoc.target)
                    .map(|s| s.as_str())
                    .unwrap_or(&assoc.target.0);
                
                writeln!(f, "└─{}{}→{}", glyph, assoc.label, target_label)?;
            }
        }
        
        Ok(())
    }
}

/// Estimate character position from token position.
/// This is a rough approximation; actual implementation would need token metadata.
/// Heuristic mapping from token index to an approximate byte offset on the line.
///
/// **IMPORTANT: This is a TEMPORARY approximation for Gate 3.**
///
/// This function does NOT have access to actual token metadata from the document.
/// It uses a simple heuristic: split by whitespace and estimate position based
/// on the token index divided by 2 (rough approximation for word + space pairs).
///
/// Known limitations:
/// - Uses byte offsets via `len()`, not char counts - may be inaccurate for non-ASCII
/// - Token indices from `SnapshotDocPos` may not align with `split_whitespace()`
/// - The `/2` heuristic is approximate and may produce visually off overlays
///
/// TODO(Gate 5/6): When real token metadata (start_char, end_char) is available
/// in the snapshot, replace this with direct lookup.
fn estimate_char_pos(line: &str, token_idx: usize) -> usize {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    
    if token_idx == 0 {
        return 0;
    }
    
    let mut char_pos = 0;
    for (i, token) in tokens.iter().enumerate() {
        if i >= token_idx / 2 {
            break;
        }
        char_pos += token.len() + 1; // +1 for space (uses byte len, not char count)
    }
    
    char_pos.min(line.len())
}

impl Snapshot {
    /// Render an annotated text view of the snapshot.
    ///
    /// This includes all span types and shows their positions overlaid on the original text.
    pub fn render_annotated(&self) -> String {
        format!("{}", DocDisplay::new(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_to_label() {
        assert_eq!(index_to_label(0), "[A]");
        assert_eq!(index_to_label(1), "[B]");
        assert_eq!(index_to_label(25), "[Z]");
        assert_eq!(index_to_label(26), "[AA]");
        assert_eq!(index_to_label(27), "[AB]");
    }

    #[test]
    fn test_doc_display_empty_snapshot() {
        let snapshot = Snapshot::new();
        let display = DocDisplay::new(&snapshot);
        let output = format!("{}", display);
        assert!(output.is_empty() || output.trim().is_empty());
    }

    #[test]
    fn test_doc_display_with_line_numbers() {
        let mut snapshot = Snapshot::with_inline_input(vec![
            "Line one".to_string(),
            "Line two".to_string(),
        ]);
        
        let display = DocDisplay::new(&snapshot);
        let output = format!("{}", display);
        
        // Should contain line numbers
        assert!(output.contains("1│"), "Should have line number 1");
        assert!(output.contains("2│"), "Should have line number 2");
    }

    #[test]
    fn test_doc_display_without_line_numbers() {
        let snapshot = Snapshot::with_inline_input(vec![
            "Line one".to_string(),
        ]);
        
        let display = DocDisplay::new(&snapshot).without_line_numbers();
        let output = format!("{}", display);
        
        // Should NOT contain line number marker
        assert!(!output.contains("│"), "Should not have line number separator");
    }

    #[test]
    fn test_doc_display_type_filtering() {
        let mut snapshot = Snapshot::with_inline_input(vec![
            "Section 1.1 Test".to_string(),
        ]);
        
        snapshot.spans.insert("SectionHeader".to_string(), vec![
            SpanData {
                id: SnapshotSpanId::new("sh", 0),
                position: super::super::types::SnapshotDocSpan::new(
                    super::super::types::SnapshotDocPos::new(0, 0),
                    super::super::types::SnapshotDocPos::new(0, 3),
                ),
                type_name: "SectionHeader".to_string(),
                value: ron::Value::String("1.1".to_string()),
                confidence: None,
                source: None,
                associations: vec![],
            },
        ]);
        
        snapshot.spans.insert("OtherType".to_string(), vec![
            SpanData {
                id: SnapshotSpanId::new("ot", 0),
                position: super::super::types::SnapshotDocSpan::new(
                    super::super::types::SnapshotDocPos::new(0, 4),
                    super::super::types::SnapshotDocPos::new(0, 5),
                ),
                type_name: "OtherType".to_string(),
                value: ron::Value::String("test".to_string()),
                confidence: None,
                source: None,
                associations: vec![],
            },
        ]);
        
        // Include only SectionHeader
        let display = DocDisplay::new(&snapshot).include("SectionHeader");
        let output = format!("{}", display);
        
        assert!(output.contains("[sh-0]"), "Should include SectionHeader");
        assert!(!output.contains("[ot-0]"), "Should exclude OtherType");
    }

    #[test]
    fn test_render_annotated_convenience() {
        let snapshot = Snapshot::with_inline_input(vec![
            "Test line".to_string(),
        ]);
        
        let output = snapshot.render_annotated();
        assert!(output.contains("Test line"));
    }
}
