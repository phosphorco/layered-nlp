use super::association::AssociatedSpan;
use super::*;
use std::collections::HashMap;
use unicode_width::UnicodeWidthStr;

/// Convert a zero-based index to a base-26 label: A, B, ..., Z, AA, AB, ..., AZ, BA, ...
/// Similar to Excel column naming.
fn index_to_base26_label(mut n: usize) -> String {
    let mut result = String::new();
    loop {
        let remainder = n % 26;
        result.insert(0, (b'A' + remainder as u8) as char);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    result
}

/// Internal representation of an included attribute for display.
struct IncludedAttr {
    range: LRange,
    debug_value: String,
    associations: Vec<AssociatedSpan>,
    show_associations: bool,
}

pub struct LLLineDisplay<'a> {
    ll_line: &'a LLLine,
    include_attrs: Vec<IncludedAttr>,
}

// 0,  1,     2,   3, - LRange indexes
// 0,  1,     5,   6, - LLToken::pos_starts_at indexes
// 1,  5,     6,   8, - LLToken::pos_ends_at indexes
// $   1000   .    00
//                ╰NATN
//            ╰PUNC
//     ╰NATN
// ╰PUNC
//     ╰────────────╯ Amount()
// ╰────────────────╯ Money($, Num)
//
// 0,  1,     2,   3, - LRange indexes
// 0,  1,     5,   6, - LLToken::pos_starts_at indexes
// 1,  5,     6,   8, - LLToken::pos_ends_at indexes
// _   1000   .    00    ;    123
//                            ╰NATN
//                       ╰PUNC
//                 ╰NATN
//            ╰PUNC
//     ╰NATN
// ╰SPACE
//     ╰────────────╯ Amount(..)
//                            ╰─╯ Amount(..)
impl<'a> std::fmt::Display for LLLineDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const SPACE_PADDING: usize = 2;
        let mut token_idx_to_start_display_char_idx = Vec::new();
        let mut token_idx_to_end_display_char_idx = Vec::new();
        // write opening display text
        let mut opening_line = String::new();
        {
            // for skipping padding at beginning
            let mut is_first = true;
            for ll_token in self.ll_line.ll_tokens.iter() {
                if is_first {
                    is_first = false;
                } else {
                    opening_line.extend(std::iter::repeat(' ').take(SPACE_PADDING));
                }

                token_idx_to_start_display_char_idx.push(UnicodeWidthStr::width(&*opening_line));

                match &ll_token.token {
                    LToken::Text(text, _) => {
                        opening_line.push_str(text);
                    }
                    LToken::Value { .. } => {
                        write!(&mut opening_line, "<>")?;
                    }
                }

                token_idx_to_end_display_char_idx.push(UnicodeWidthStr::width(&*opening_line));
            }
        }

        f.write_str(&opening_line)?;

        // Build span label map for referenced spans: LRange -> "[A]", "[B]", etc.
        // Only assign labels to spans that are included in the display
        let span_labels = self.build_span_labels();

        // ex:
        //     ╰────────────╯[A] Amount(..)
        //                            ╰─╯ Amount(..)
        //                       └─@source─>[A]
        for attr in self.include_attrs.iter() {
            f.write_char('\n')?;

            let start_char_idx = token_idx_to_start_display_char_idx[attr.range.0];
            for _ in 0..start_char_idx {
                f.write_char(' ')?;
            }

            f.write_char('╰')?;

            let end_char_idx = token_idx_to_end_display_char_idx[attr.range.1];
            let char_len = end_char_idx - start_char_idx;
            for _ in (start_char_idx + 1)..end_char_idx.saturating_sub(1) {
                f.write_char('─')?;
            }

            if char_len > 1 {
                f.write_char('╯')?;
            }

            // Add span label if this range is referenced by associations
            if let Some(label) = span_labels.get(&attr.range) {
                write!(f, "{} ", label)?;
            }

            f.write_str(&attr.debug_value)?;

            // Render association arrows if enabled
            if attr.show_associations && !attr.associations.is_empty() {
                for assoc in &attr.associations {
                    f.write_char('\n')?;

                    // Indent to align with the span start, plus a small offset
                    let arrow_indent = start_char_idx + 2;
                    for _ in 0..arrow_indent {
                        f.write_char(' ')?;
                    }

                    // Build the arrow: └─{glyph}{label}─>[target]
                    let glyph = assoc.glyph().unwrap_or("");
                    let label = assoc.label();
                    let target_range = (assoc.span.start_idx, assoc.span.end_idx);

                    let target_str = if let Some(target_label) = span_labels.get(&target_range) {
                        target_label.clone()
                    } else {
                        format!("[{}..{}]", target_range.0, target_range.1)
                    };

                    write!(f, "└─{}{}─>{}", glyph, label, target_str)?;
                }
            }
        }

        Ok(())
    }
}

impl<'a> LLLineDisplay<'a> {
    pub fn new(ll_line: &'a LLLine) -> Self {
        LLLineDisplay {
            ll_line,
            include_attrs: Vec::new(),
        }
    }

    /// Build a map from included ranges to labels like "[A]", "[B]", etc.
    /// Only ranges that are targets of associations get labels.
    fn build_span_labels(&self) -> HashMap<LRange, String> {
        // Collect all target ranges from associations
        let mut target_ranges: Vec<LRange> = self
            .include_attrs
            .iter()
            .filter(|attr| attr.show_associations)
            .flat_map(|attr| &attr.associations)
            .map(|assoc| (assoc.span.start_idx, assoc.span.end_idx))
            .collect();

        // Filter to only ranges that are actually included in the display
        let included_ranges: std::collections::HashSet<LRange> =
            self.include_attrs.iter().map(|attr| attr.range).collect();
        target_ranges.retain(|range| included_ranges.contains(range));

        // Remove duplicates and sort deterministically by (start, end)
        target_ranges.sort();
        target_ranges.dedup();

        // Assign labels: [A], [B], ..., [Z], [AA], [AB], etc.
        target_ranges
            .into_iter()
            .enumerate()
            .map(|(i, range)| {
                let label = format!("[{}]", index_to_base26_label(i));
                (range, label)
            })
            .collect()
    }

    // TODO consider making this method take and return `self`
    pub fn include<T: 'static + std::fmt::Debug>(&mut self) {
        for ll_range in self.ll_line.attrs.ranges.get::<T>() {
            for debug_value in self
                .ll_line
                .attrs
                .values
                .get(ll_range)
                .into_iter()
                .flat_map(|type_bucket| type_bucket.get_debug::<T>())
                .rev()
            {
                self.include_attrs.push(IncludedAttr {
                    range: *ll_range,
                    debug_value,
                    associations: Vec::new(),
                    show_associations: false,
                });
            }
        }
    }

    /// Include attributes of type T with their associations rendered.
    ///
    /// This is similar to [`include`](Self::include) but also renders
    /// association arrows below each attribute span.
    pub fn include_with_associations<T: 'static + std::fmt::Debug>(&mut self) {
        for ll_range in self.ll_line.attrs.ranges.get::<T>() {
            for (debug_value, associations) in self
                .ll_line
                .attrs
                .values
                .get(ll_range)
                .into_iter()
                .flat_map(|type_bucket| type_bucket.get_debug_with_associations::<T>())
                .rev()
            {
                self.include_attrs.push(IncludedAttr {
                    range: *ll_range,
                    debug_value,
                    associations,
                    show_associations: true,
                });
            }
        }
    }

    /// Takes self
    pub fn with<T: 'static + std::fmt::Debug>(mut self) -> Self {
        self.include::<T>();
        self
    }

    /// Takes self, includes associations
    pub fn with_associations<T: 'static + std::fmt::Debug>(mut self) -> Self {
        self.include_with_associations::<T>();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_to_base26_label() {
        // First 26 labels: A-Z
        assert_eq!(index_to_base26_label(0), "A");
        assert_eq!(index_to_base26_label(1), "B");
        assert_eq!(index_to_base26_label(25), "Z");

        // Next 26 labels: AA-AZ
        assert_eq!(index_to_base26_label(26), "AA");
        assert_eq!(index_to_base26_label(27), "AB");
        assert_eq!(index_to_base26_label(51), "AZ");

        // Next 26 labels: BA-BZ
        assert_eq!(index_to_base26_label(52), "BA");
        assert_eq!(index_to_base26_label(77), "BZ");

        // ZZ is at 26 + 26*26 - 1 = 701
        assert_eq!(index_to_base26_label(701), "ZZ");

        // AAA is at 26 + 26*26 = 702
        assert_eq!(index_to_base26_label(702), "AAA");
    }
}
