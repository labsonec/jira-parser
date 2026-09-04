use crate::ast::{Block, Document, Inline, ListItem, TableCell, TableRow};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    UnclosedCodeBlock { start_line: usize },
    UnclosedNoFormatBlock { start_line: usize },
    UnclosedQuoteBlock { start_line: usize },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::UnclosedCodeBlock { start_line } => {
                write!(f, "unclosed code block starting on line {start_line}")
            }
            ParseError::UnclosedNoFormatBlock { start_line } => {
                write!(f, "unclosed noformat block starting on line {start_line}")
            }
            ParseError::UnclosedQuoteBlock { start_line } => {
                write!(f, "unclosed quote block starting on line {start_line}")
            }
        }
    }
}

impl std::error::Error for ParseError {}

pub fn parse_jira(input: &str) -> Result<Document, ParseError> {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let (blocks, _, _) = parse_blocks(&lines, 0, false)?;

    Ok(Document { blocks })
}

fn parse_blocks(
    lines: &[&str],
    start: usize,
    stop_at_quote_close: bool,
) -> Result<(Vec<Block>, usize, bool), ParseError> {
    let mut blocks = Vec::new();
    let mut index = start;

    while index < lines.len() {
        let line = lines[index];
        let stripped_line = strip_leading_space(line);

        if line.trim().is_empty() {
            index += 1;
            continue;
        }

        if stop_at_quote_close && is_quote_fence(stripped_line) {
            return Ok((blocks, index + 1, true));
        }

        if stripped_line == "{code}" {
            let start_line = index + 1;
            let mut content = Vec::new();
            let mut closed = false;
            index += 1;

            while index < lines.len() {
                let code_line = lines[index];
                if strip_leading_space(code_line) == "{code}" {
                    blocks.push(Block::CodeBlock {
                        text: content.join("\n"),
                    });
                    index += 1;
                    closed = true;
                    break;
                }

                content.push(code_line);
                index += 1;
            }

            if !closed {
                return Err(ParseError::UnclosedCodeBlock { start_line });
            }

            continue;
        }

        if stripped_line == "{noformat}" {
            let start_line = index + 1;
            let mut content = Vec::new();
            let mut closed = false;
            index += 1;

            while index < lines.len() {
                let noformat_line = lines[index];
                if strip_leading_space(noformat_line) == "{noformat}" {
                    blocks.push(Block::NoFormatBlock {
                        text: content.join("\n"),
                    });
                    index += 1;
                    closed = true;
                    break;
                }

                content.push(noformat_line);
                index += 1;
            }

            if !closed {
                return Err(ParseError::UnclosedNoFormatBlock { start_line });
            }

            continue;
        }

        if let Some(text) = parse_line_quote(stripped_line) {
            blocks.push(Block::Quote {
                blocks: vec![Block::Paragraph {
                    content: parse_inlines(text),
                }],
            });
            index += 1;
            continue;
        }

        if is_quote_fence(stripped_line) {
            let start_line = index + 1;
            let (quote_blocks, next_index, closed) = parse_blocks(lines, index + 1, true)?;
            if !closed {
                return Err(ParseError::UnclosedQuoteBlock { start_line });
            }

            blocks.push(Block::Quote {
                blocks: quote_blocks,
            });
            index = next_index;
            continue;
        }

        if let Some((level, text)) = parse_heading(stripped_line) {
            blocks.push(Block::Heading {
                level,
                content: parse_inlines(text),
            });
            index += 1;
            continue;
        }

        if parse_list_marker(stripped_line).is_some() {
            let (list_blocks, next_index) = parse_list_blocks(&lines, index);
            blocks.extend(list_blocks);
            index = next_index;
            continue;
        }

        if let Some((table, next_index)) = parse_table(&lines, index) {
            blocks.push(table);
            index = next_index;
            continue;
        }

        let (text, next_index) = parse_paragraph(lines, index);
        blocks.push(Block::Paragraph {
            content: parse_inlines(&text),
        });
        index = next_index;
    }

    Ok((blocks, index, false))
}

fn parse_inlines(text: &str) -> Vec<Inline> {
    let mut parser = InlineParser::new(text, false, None);
    parser.parse()
}

fn parse_inlines_after_style_opener(text: &str, parent_style_delimiter: u8) -> Vec<Inline> {
    let mut parser = InlineParser::new(text, true, Some(parent_style_delimiter));
    parser.parse()
}

struct InlineParser<'a> {
    text: &'a str,
    index: usize,
    inlines: Vec<Inline>,
    after_style_opener: bool,
    parent_style_delimiter: Option<u8>,
}

impl<'a> InlineParser<'a> {
    fn new(text: &'a str, after_style_opener: bool, parent_style_delimiter: Option<u8>) -> Self {
        Self {
            text,
            index: 0,
            inlines: Vec::new(),
            after_style_opener,
            parent_style_delimiter,
        }
    }

    fn parse(&mut self) -> Vec<Inline> {
        while self.index < self.text.len() {
            let remaining = &self.text[self.index..];

            if self.try_parse_color() || self.try_parse_link() {
                continue;
            }

            let byte = remaining.as_bytes()[0];
            if matches!(byte, b'*' | b'_' | b'+' | b'-')
                && self.parent_style_delimiter != Some(byte)
                && self.try_parse_delimited(byte)
            {
                continue;
            }

            self.push_next_char();
        }

        std::mem::take(&mut self.inlines)
    }

    fn try_parse_delimited(&mut self, delimiter: u8) -> bool {
        let remaining = &self.text[self.index..];
        let content_start = self.index + 1;
        let candidate_ends: Vec<usize> = remaining[1..]
            .match_indices(delimiter as char)
            .map(|(relative_end, _)| content_start + relative_end)
            .collect();

        for &content_end in &candidate_ends {
            if !self.is_valid_delimited_close(content_start, content_end, true) {
                continue;
            }

            self.push_delimited_inline(delimiter, content_start, content_end);
            return true;
        }

        if !self.after_style_opener {
            for content_end in candidate_ends.into_iter().rev() {
                if !self.is_valid_delimited_close(content_start, content_end, false) {
                    continue;
                }

                self.push_delimited_inline(delimiter, content_start, content_end);
                return true;
            }
        }

        false
    }

    fn push_delimited_inline(&mut self, delimiter: u8, content_start: usize, content_end: usize) {
        let content =
            parse_inlines_after_style_opener(&self.text[content_start..content_end], delimiter);
        let inline = match delimiter {
            b'*' => Inline::Strong(content),
            b'_' => Inline::Emphasis(content),
            b'+' => Inline::Inserted(content),
            b'-' => Inline::Strikethrough(content),
            _ => unreachable!("delimiter is checked by caller"),
        };

        self.inlines.push(inline);
        self.index = content_end + 1;
    }

    fn is_valid_delimited_close(
        &self,
        content_start: usize,
        content_end: usize,
        require_boundary: bool,
    ) -> bool {
        if content_start == content_end {
            return false;
        }

        if !require_boundary {
            return true;
        }

        let close_end = content_end + 1;
        self.text[close_end..]
            .chars()
            .next()
            .is_none_or(|character| {
                character.is_whitespace() || matches!(character, '*' | '_' | '+' | '-')
            })
    }

    fn try_parse_color(&mut self) -> bool {
        const OPEN_PREFIX: &str = "{color:";
        const CLOSE: &str = "{color}";

        let remaining = &self.text[self.index..];
        if !remaining.starts_with(OPEN_PREFIX) {
            return false;
        }

        let Some(relative_open_end) = remaining.find('}') else {
            self.push_text(remaining);
            self.index = self.text.len();
            return true;
        };

        let color_start = self.index + OPEN_PREFIX.len();
        let color_end = self.index + relative_open_end;
        if color_start == color_end {
            return false;
        }

        let content_start = color_end + 1;
        let after_open = &self.text[content_start..];
        let Some(relative_close_start) = after_open.find(CLOSE) else {
            self.push_text(remaining);
            self.index = self.text.len();
            return true;
        };

        let content_end = content_start + relative_close_start;
        self.inlines.push(Inline::Color {
            color: self.text[color_start..color_end].to_string(),
            content: parse_inlines(&self.text[content_start..content_end]),
        });
        self.index = content_end + CLOSE.len();
        true
    }

    fn try_parse_link(&mut self) -> bool {
        let remaining = &self.text[self.index..];
        if !remaining.starts_with('[') {
            return false;
        }

        let Some(relative_end) = remaining[1..].find(']') else {
            return false;
        };

        let content_start = self.index + 1;
        let content_end = content_start + relative_end;
        let content = &self.text[content_start..content_end];
        let inline = if let Some((label, url)) = content.split_once('|') {
            if !is_supported_url(url) {
                self.push_text(&self.text[self.index..=content_end]);
                self.index = content_end + 1;
                return true;
            }

            Inline::Link {
                text: Some(parse_inlines(label)),
                url: url.to_string(),
            }
        } else {
            if !is_supported_url(content) {
                self.push_text(&self.text[self.index..=content_end]);
                self.index = content_end + 1;
                return true;
            }

            Inline::Link {
                text: None,
                url: content.to_string(),
            }
        };

        self.inlines.push(inline);
        self.index = content_end + 1;
        true
    }

    fn push_next_char(&mut self) {
        let next = self.text[self.index..]
            .chars()
            .next()
            .expect("index is inside text");
        self.push_text(&self.text[self.index..self.index + next.len_utf8()]);
        self.index += next.len_utf8();
    }

    fn push_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        if let Some(Inline::Text(existing)) = self.inlines.last_mut() {
            existing.push_str(text);
        } else {
            self.inlines.push(Inline::Text(text.to_string()));
        }
    }
}

fn is_supported_url(url: &str) -> bool {
    url.strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .is_some_and(|rest| !rest.is_empty())
}

fn parse_heading(line: &str) -> Option<(u8, &str)> {
    let bytes = line.as_bytes();
    if bytes.len() >= 4
        && bytes[0] == b'h'
        && (b'1'..=b'6').contains(&bytes[1])
        && bytes[2] == b'.'
        && bytes[3] == b' '
    {
        Some((bytes[1] - b'0', &line[4..]))
    } else {
        None
    }
}

fn parse_paragraph(lines: &[&str], start: usize) -> (String, usize) {
    let mut text = String::new();
    let mut index = start;

    while index < lines.len() {
        let line = lines[index];
        if line.trim().is_empty() || is_block_start(line) {
            break;
        }

        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(strip_leading_space(line));
        index += 1;
    }

    (text, index)
}

fn is_block_start(line: &str) -> bool {
    let stripped_line = strip_leading_space(line);

    stripped_line == "{code}"
        || stripped_line == "{noformat}"
        || is_quote_fence(stripped_line)
        || parse_line_quote(stripped_line).is_some()
        || parse_heading(stripped_line).is_some()
        || parse_list_marker(stripped_line).is_some()
        || parse_table_row(stripped_line).is_some()
}

fn is_quote_fence(line: &str) -> bool {
    line == "{quote}"
}

fn parse_line_quote(line: &str) -> Option<&str> {
    line.strip_prefix("bq. ")
}

fn strip_leading_space(line: &str) -> &str {
    line.trim_start_matches([' ', '\t'])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListKind {
    Ordered,
    Unordered,
}

struct ListMarker<'a> {
    path: Vec<ListKind>,
    text: &'a str,
}

fn parse_list_marker(line: &str) -> Option<ListMarker<'_>> {
    let line = strip_leading_space(line);
    let marker_len = line
        .bytes()
        .take_while(|byte| matches!(byte, b'#' | b'-'))
        .count();

    if marker_len == 0 {
        return None;
    }

    let text = line.get(marker_len..)?;
    let whitespace_len = text
        .bytes()
        .take_while(|byte| byte.is_ascii_whitespace())
        .count();

    if whitespace_len == 0 {
        return None;
    }

    let path = line[..marker_len]
        .bytes()
        .map(|byte| match byte {
            b'#' => ListKind::Ordered,
            b'-' => ListKind::Unordered,
            _ => unreachable!("marker bytes are filtered before mapping"),
        })
        .collect();

    Some(ListMarker {
        path,
        text: &text[whitespace_len..],
    })
}

fn parse_list_blocks(lines: &[&str], start: usize) -> (Vec<Block>, usize) {
    parse_list_blocks_at(lines, start, &[])
}

fn parse_list_blocks_at(
    lines: &[&str],
    start: usize,
    parent_path: &[ListKind],
) -> (Vec<Block>, usize) {
    let mut blocks = Vec::new();
    let mut current_kind = None;
    let mut current_items = Vec::new();
    let mut index = start;

    while index < lines.len() {
        let Some(marker) = parse_list_marker(lines[index]) else {
            break;
        };

        if marker.path.len() <= parent_path.len() || !marker.path.starts_with(parent_path) {
            break;
        }

        let item_kind = marker.path[parent_path.len()];
        if current_kind.is_some_and(|kind| kind != item_kind) {
            blocks.push(list_block(current_kind.unwrap(), current_items));
            current_items = Vec::new();
        }
        current_kind = Some(item_kind);

        let mut item = ListItem {
            blocks: vec![Block::Paragraph {
                content: parse_inlines(marker.text),
            }],
        };
        let item_path = marker.path;
        index += 1;

        let (nested_blocks, next_index) = parse_list_blocks_at(lines, index, &item_path);
        item.blocks.extend(nested_blocks);
        index = next_index;

        current_items.push(item);
    }

    if let Some(kind) = current_kind {
        blocks.push(list_block(kind, current_items));
    }

    (blocks, index)
}

fn list_block(kind: ListKind, items: Vec<ListItem>) -> Block {
    match kind {
        ListKind::Ordered => Block::OrderedList { items },
        ListKind::Unordered => Block::UnorderedList { items },
    }
}

fn parse_table(lines: &[&str], start: usize) -> Option<(Block, usize)> {
    let mut rows = Vec::new();
    let mut index = start;

    while index < lines.len() {
        let Some(row) = parse_table_row(strip_leading_space(lines[index])) else {
            break;
        };

        rows.push(row);
        index += 1;
    }

    if rows.is_empty() {
        None
    } else {
        Some((Block::Table { rows }, index))
    }
}

fn parse_table_row(line: &str) -> Option<TableRow> {
    let (header, delimiter) = if line.starts_with("||") {
        (true, "||")
    } else if line.starts_with('|') {
        (false, "|")
    } else {
        return None;
    };

    if line.len() == delimiter.len() {
        return None;
    }

    let cells = split_table_cells(&line[delimiter.len()..], delimiter)?
        .into_iter()
        .map(|cell| TableCell {
            header,
            content: parse_inlines(cell.trim()),
        })
        .collect();

    Some(TableRow { cells })
}

fn split_table_cells<'a>(content: &'a str, delimiter: &str) -> Option<Vec<&'a str>> {
    let mut cells = Vec::new();
    let mut cell_start = 0;
    let mut index = 0;
    let mut bracket_depth = 0usize;

    while index < content.len() {
        let remaining = &content[index..];

        if bracket_depth == 0 && remaining.starts_with(delimiter) {
            cells.push(&content[cell_start..index]);
            index += delimiter.len();
            cell_start = index;
            continue;
        }

        let character = remaining
            .chars()
            .next()
            .expect("index is inside table row content");
        match character {
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            _ => {}
        }
        index += character.len_utf8();
    }

    if cell_start != content.len() {
        return None;
    }

    Some(cells)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_text(text: impl Into<String>) -> Vec<Inline> {
        vec![Inline::Text(text.into())]
    }

    #[test]
    fn parses_heading() {
        let document = parse_jira("h2. Release notes").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Heading {
                    level: 2,
                    content: raw_text("Release notes"),
                }],
            }
        );
    }

    #[test]
    fn parses_single_line_paragraph() {
        let document = parse_jira("A short paragraph.").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Paragraph {
                    content: raw_text("A short paragraph."),
                }],
            }
        );
    }

    #[test]
    fn parses_multi_line_paragraph_joined_with_newlines() {
        let document = parse_jira("First line\nsecond line\nthird line").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Paragraph {
                    content: raw_text("First line\nsecond line\nthird line"),
                }],
            }
        );
    }

    #[test]
    fn strips_leading_spaces_from_paragraph_lines() {
        let document = parse_jira("  First line\n\tsecond line\nthird line").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Paragraph {
                    content: raw_text("First line\nsecond line\nthird line"),
                }],
            }
        );
    }

    #[test]
    fn strips_leading_spaces_before_headings() {
        let document = parse_jira("\t h3. Indented heading").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Heading {
                    level: 3,
                    content: raw_text("Indented heading"),
                }],
            }
        );
    }

    #[test]
    fn parses_ordered_list() {
        let document = parse_jira("# First\n# Second").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::OrderedList {
                    items: vec![
                        ListItem {
                            blocks: vec![Block::Paragraph {
                                content: raw_text("First"),
                            }],
                        },
                        ListItem {
                            blocks: vec![Block::Paragraph {
                                content: raw_text("Second"),
                            }],
                        },
                    ],
                }],
            }
        );
    }

    #[test]
    fn accepts_leading_spaces_before_flat_list_markers() {
        let document = parse_jira("  # First\n\t# Second\n  - Apple\n\t- Banana").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![
                    Block::OrderedList {
                        items: vec![
                            ListItem {
                                blocks: vec![Block::Paragraph {
                                    content: raw_text("First"),
                                }],
                            },
                            ListItem {
                                blocks: vec![Block::Paragraph {
                                    content: raw_text("Second"),
                                }],
                            },
                        ],
                    },
                    Block::UnorderedList {
                        items: vec![
                            ListItem {
                                blocks: vec![Block::Paragraph {
                                    content: raw_text("Apple"),
                                }],
                            },
                            ListItem {
                                blocks: vec![Block::Paragraph {
                                    content: raw_text("Banana"),
                                }],
                            },
                        ],
                    },
                ],
            }
        );
    }

    #[test]
    fn parses_unordered_list() {
        let document = parse_jira("- First\n- Second").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::UnorderedList {
                    items: vec![
                        ListItem {
                            blocks: vec![Block::Paragraph {
                                content: raw_text("First"),
                            }],
                        },
                        ListItem {
                            blocks: vec![Block::Paragraph {
                                content: raw_text("Second"),
                            }],
                        },
                    ],
                }],
            }
        );
    }

    #[test]
    fn parses_nested_unordered_list() {
        let document = parse_jira("- Parent\n-- Chicago\n-- Phoenix\n- Sibling").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::UnorderedList {
                    items: vec![
                        ListItem {
                            blocks: vec![
                                Block::Paragraph {
                                    content: raw_text("Parent"),
                                },
                                Block::UnorderedList {
                                    items: vec![
                                        ListItem {
                                            blocks: vec![Block::Paragraph {
                                                content: raw_text("Chicago"),
                                            }],
                                        },
                                        ListItem {
                                            blocks: vec![Block::Paragraph {
                                                content: raw_text("Phoenix"),
                                            }],
                                        },
                                    ],
                                },
                            ],
                        },
                        ListItem {
                            blocks: vec![Block::Paragraph {
                                content: raw_text("Sibling"),
                            }],
                        },
                    ],
                }],
            }
        );
    }

    #[test]
    fn parses_nested_ordered_list_under_unordered_item() {
        let document = parse_jira("- Parent\n-# load\n-# fire\n- Sibling").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::UnorderedList {
                    items: vec![
                        ListItem {
                            blocks: vec![
                                Block::Paragraph {
                                    content: raw_text("Parent"),
                                },
                                Block::OrderedList {
                                    items: vec![
                                        ListItem {
                                            blocks: vec![Block::Paragraph {
                                                content: raw_text("load"),
                                            }],
                                        },
                                        ListItem {
                                            blocks: vec![Block::Paragraph {
                                                content: raw_text("fire"),
                                            }],
                                        },
                                    ],
                                },
                            ],
                        },
                        ListItem {
                            blocks: vec![Block::Paragraph {
                                content: raw_text("Sibling"),
                            }],
                        },
                    ],
                }],
            }
        );
    }

    #[test]
    fn leaves_marker_runs_without_delimiter_whitespace_as_paragraph_text() {
        let document = parse_jira("-#foo").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Paragraph {
                    content: raw_text("-#foo"),
                }],
            }
        );
    }

    #[test]
    fn parses_user_nested_list_example() {
        let document = parse_jira(
            "this is the top level unorders list:\n  - normal list item with leading spaces before {{-}}\n- this is an unordered sublist\n-- Chicago\n-- Phoenix\n- this is an ordered sublist\n-# load\n-# fire",
        )
        .unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![
                    Block::Paragraph {
                        content: raw_text("this is the top level unorders list:"),
                    },
                    Block::UnorderedList {
                        items: vec![
                            ListItem {
                                blocks: vec![Block::Paragraph {
                                    content: raw_text(
                                        "normal list item with leading spaces before {{-}}",
                                    ),
                                }],
                            },
                            ListItem {
                                blocks: vec![
                                    Block::Paragraph {
                                        content: raw_text("this is an unordered sublist"),
                                    },
                                    Block::UnorderedList {
                                        items: vec![
                                            ListItem {
                                                blocks: vec![Block::Paragraph {
                                                    content: raw_text("Chicago"),
                                                }],
                                            },
                                            ListItem {
                                                blocks: vec![Block::Paragraph {
                                                    content: raw_text("Phoenix"),
                                                }],
                                            },
                                        ],
                                    },
                                ],
                            },
                            ListItem {
                                blocks: vec![
                                    Block::Paragraph {
                                        content: raw_text("this is an ordered sublist"),
                                    },
                                    Block::OrderedList {
                                        items: vec![
                                            ListItem {
                                                blocks: vec![Block::Paragraph {
                                                    content: raw_text("load"),
                                                }],
                                            },
                                            ListItem {
                                                blocks: vec![Block::Paragraph {
                                                    content: raw_text("fire"),
                                                }],
                                            },
                                        ],
                                    },
                                ],
                            },
                        ],
                    },
                ],
            }
        );
    }

    #[test]
    fn preserves_leading_spaces_in_code_and_noformat_blocks() {
        let document = parse_jira(
            "  {code}\n  let value = 1;\n\tprintln!(\"{value}\");\n\t{code}\n  {noformat}\n  first line\n\tsecond line\n  {noformat}",
        )
        .unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![
                    Block::CodeBlock {
                        text: "  let value = 1;\n\tprintln!(\"{value}\");".to_string(),
                    },
                    Block::NoFormatBlock {
                        text: "  first line\n\tsecond line".to_string(),
                    },
                ],
            }
        );
    }

    #[test]
    fn parses_code_block() {
        let document =
            parse_jira("{code}\nlet value = 1;\nprintln!(\"{value}\");\n{code}").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::CodeBlock {
                    text: "let value = 1;\nprintln!(\"{value}\");".to_string(),
                }],
            }
        );
    }

    #[test]
    fn reports_unclosed_code_block() {
        let error = parse_jira("intro\n{code}\nlet value = 1;").unwrap_err();

        assert_eq!(error, ParseError::UnclosedCodeBlock { start_line: 2 });
    }

    #[test]
    fn parses_noformat_block() {
        let document = parse_jira(
            "before\n{noformat}\n  first line\n\tsecond line\n# not a list\n{noformat}\nafter",
        )
        .unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![
                    Block::Paragraph {
                        content: raw_text("before"),
                    },
                    Block::NoFormatBlock {
                        text: "  first line\n\tsecond line\n# not a list".to_string(),
                    },
                    Block::Paragraph {
                        content: raw_text("after"),
                    },
                ],
            }
        );
    }

    #[test]
    fn reports_unclosed_noformat_block() {
        let error = parse_jira("intro\n{noformat}\n  value").unwrap_err();

        assert_eq!(error, ParseError::UnclosedNoFormatBlock { start_line: 2 });
    }

    #[test]
    fn parses_line_quote_with_inline_markup() {
        let document = parse_jira("bq. quoted *strong* text").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Quote {
                    blocks: vec![Block::Paragraph {
                        content: vec![
                            Inline::Text("quoted ".to_string()),
                            Inline::Strong(raw_text("strong")),
                            Inline::Text(" text".to_string()),
                        ],
                    }],
                }],
            }
        );
    }

    #[test]
    fn parses_fenced_quote_paragraph() {
        let document = parse_jira("{quote}\nquoted\n{quote}").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Quote {
                    blocks: vec![Block::Paragraph {
                        content: raw_text("quoted"),
                    }],
                }],
            }
        );
    }

    #[test]
    fn parses_fenced_quote_with_nested_list_and_table() {
        let document =
            parse_jira("{quote}\n- item *one*\n||Name||Status||\n|Build|Green|\n{quote}").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Quote {
                    blocks: vec![
                        Block::UnorderedList {
                            items: vec![ListItem {
                                blocks: vec![Block::Paragraph {
                                    content: vec![
                                        Inline::Text("item ".to_string()),
                                        Inline::Strong(raw_text("one")),
                                    ],
                                }],
                            }],
                        },
                        Block::Table {
                            rows: vec![
                                TableRow {
                                    cells: vec![
                                        TableCell {
                                            header: true,
                                            content: raw_text("Name"),
                                        },
                                        TableCell {
                                            header: true,
                                            content: raw_text("Status"),
                                        },
                                    ],
                                },
                                TableRow {
                                    cells: vec![
                                        TableCell {
                                            header: false,
                                            content: raw_text("Build"),
                                        },
                                        TableCell {
                                            header: false,
                                            content: raw_text("Green"),
                                        },
                                    ],
                                },
                            ],
                        },
                    ],
                }],
            }
        );
    }

    #[test]
    fn reports_unclosed_quote_block() {
        let error = parse_jira("intro\n{quote}\nquoted").unwrap_err();

        assert_eq!(error, ParseError::UnclosedQuoteBlock { start_line: 2 });
    }

    #[test]
    fn preserves_quote_markers_inside_code_and_noformat_blocks() {
        let document = parse_jira(
            "{quote}\n{code}\n{quote}\nbq. quoted\n{code}\n{noformat}\n{quote}\nbq. quoted\n{noformat}\n{quote}",
        )
        .unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Quote {
                    blocks: vec![
                        Block::CodeBlock {
                            text: "{quote}\nbq. quoted".to_string(),
                        },
                        Block::NoFormatBlock {
                            text: "{quote}\nbq. quoted".to_string(),
                        },
                    ],
                }],
            }
        );
    }

    #[test]
    fn parses_header_only_table_row() {
        let document = parse_jira("  || heading 1 ||heading 2||").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Table {
                    rows: vec![TableRow {
                        cells: vec![
                            TableCell {
                                header: true,
                                content: raw_text("heading 1"),
                            },
                            TableCell {
                                header: true,
                                content: raw_text("heading 2"),
                            },
                        ],
                    }],
                }],
            }
        );
    }

    #[test]
    fn parses_table_with_header_and_body_rows() {
        let document = parse_jira("||Name||Status||\n|Build|Green|").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Table {
                    rows: vec![
                        TableRow {
                            cells: vec![
                                TableCell {
                                    header: true,
                                    content: raw_text("Name"),
                                },
                                TableCell {
                                    header: true,
                                    content: raw_text("Status"),
                                },
                            ],
                        },
                        TableRow {
                            cells: vec![
                                TableCell {
                                    header: false,
                                    content: raw_text("Build"),
                                },
                                TableCell {
                                    header: false,
                                    content: raw_text("Green"),
                                },
                            ],
                        },
                    ],
                }],
            }
        );
    }

    #[test]
    fn parses_inline_content_inside_table_cells() {
        let document = parse_jira("|*strong*|[Atlassian|https://atlassian.com]|").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Table {
                    rows: vec![TableRow {
                        cells: vec![
                            TableCell {
                                header: false,
                                content: vec![Inline::Strong(raw_text("strong"))],
                            },
                            TableCell {
                                header: false,
                                content: vec![Inline::Link {
                                    text: Some(raw_text("Atlassian")),
                                    url: "https://atlassian.com".to_string(),
                                }],
                            },
                        ],
                    }],
                }],
            }
        );
    }

    #[test]
    fn table_stops_before_non_table_line() {
        let document = parse_jira("|A|B|\nplain text").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![
                    Block::Table {
                        rows: vec![TableRow {
                            cells: vec![
                                TableCell {
                                    header: false,
                                    content: raw_text("A"),
                                },
                                TableCell {
                                    header: false,
                                    content: raw_text("B"),
                                },
                            ],
                        }],
                    },
                    Block::Paragraph {
                        content: raw_text("plain text"),
                    },
                ],
            }
        );
    }

    #[test]
    fn preserves_table_like_text_inside_code_and_noformat_blocks() {
        let document =
            parse_jira("{code}\n|A|B|\n{code}\n{noformat}\n||Heading||\n{noformat}").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![
                    Block::CodeBlock {
                        text: "|A|B|".to_string(),
                    },
                    Block::NoFormatBlock {
                        text: "||Heading||".to_string(),
                    },
                ],
            }
        );
    }

    #[test]
    fn normalizes_line_endings() {
        let document = parse_jira("alpha\r\nbeta\rgamma").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Paragraph {
                    content: raw_text("alpha\nbeta\ngamma"),
                }],
            }
        );
    }

    #[test]
    fn parses_delimited_inline_markup_in_paragraphs() {
        let document = parse_jira("This is *strong*, +inserted+, and -gone-.").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Paragraph {
                    content: vec![
                        Inline::Text("This is ".to_string()),
                        Inline::Strong(raw_text("strong")),
                        Inline::Text(", ".to_string()),
                        Inline::Inserted(raw_text("inserted")),
                        Inline::Text(", and ".to_string()),
                        Inline::Strikethrough(raw_text("gone")),
                        Inline::Text(".".to_string()),
                    ],
                }],
            }
        );
    }

    #[test]
    fn parses_top_level_inserted_inline_markup() {
        let document = parse_jira("+Title+").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Paragraph {
                    content: vec![Inline::Inserted(raw_text("Title"))],
                }],
            }
        );
    }

    #[test]
    fn parses_initial_adjacent_style_when_it_reaches_parent_end() {
        let document = parse_jira("*+Title+*").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Paragraph {
                    content: vec![Inline::Strong(vec![Inline::Inserted(raw_text("Title"))])],
                }],
            }
        );
    }

    #[test]
    fn parses_initial_adjacent_style_when_followed_by_whitespace() {
        let document = parse_jira("*+Title+ _normal_*").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Paragraph {
                    content: vec![Inline::Strong(vec![
                        Inline::Inserted(raw_text("Title")),
                        Inline::Text(" ".to_string()),
                        Inline::Emphasis(raw_text("normal")),
                    ])],
                }],
            }
        );
    }

    #[test]
    fn treats_style_marker_adjacent_to_style_opener_as_text() {
        let document = parse_jira("*+Title+normal*").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Paragraph {
                    content: vec![Inline::Strong(raw_text("+Title+normal"))],
                }],
            }
        );
    }

    #[test]
    fn parses_adjacent_nested_styles_inside_outer_strong() {
        let document = parse_jira("*_a_ +b+_c_*").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Paragraph {
                    content: vec![Inline::Strong(vec![
                        Inline::Emphasis(raw_text("a")),
                        Inline::Text(" ".to_string()),
                        Inline::Inserted(raw_text("b")),
                        Inline::Emphasis(raw_text("c")),
                    ])],
                }],
            }
        );
    }

    #[test]
    fn closes_outer_strong_before_trailing_duplicate_marker() {
        let document = parse_jira("*_a_ *c**").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Paragraph {
                    content: vec![
                        Inline::Strong(vec![
                            Inline::Emphasis(raw_text("a")),
                            Inline::Text(" *c".to_string()),
                        ]),
                        Inline::Text("*".to_string()),
                    ],
                }],
            }
        );
    }

    #[test]
    fn closes_inner_inserted_before_duplicate_plus_inside_strong() {
        let document = parse_jira("*+a++*").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Paragraph {
                    content: vec![Inline::Strong(vec![
                        Inline::Inserted(raw_text("a")),
                        Inline::Text("+".to_string()),
                    ])],
                }],
            }
        );
    }

    #[test]
    fn ignores_plus_followed_by_normal_text_when_closing_inserted_inside_strong() {
        let document = parse_jira("*+a +b++*").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Paragraph {
                    content: vec![Inline::Strong(vec![
                        Inline::Inserted(raw_text("a +b")),
                        Inline::Text("+".to_string()),
                    ])],
                }],
            }
        );
    }

    #[test]
    fn leaves_literal_star_after_inserted_span_in_strong() {
        let document = parse_jira("*+a+ *c**").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Paragraph {
                    content: vec![
                        Inline::Strong(vec![
                            Inline::Inserted(raw_text("a")),
                            Inline::Text(" *c".to_string()),
                        ]),
                        Inline::Text("*".to_string()),
                    ],
                }],
            }
        );
    }

    #[test]
    fn parses_emphasis_and_inserted_with_duplicate_plus_inside_strong() {
        let document = parse_jira("*_a_ +b +c++*").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Paragraph {
                    content: vec![Inline::Strong(vec![
                        Inline::Emphasis(raw_text("a")),
                        Inline::Text(" ".to_string()),
                        Inline::Inserted(raw_text("b +c")),
                        Inline::Text("+".to_string()),
                    ])],
                }],
            }
        );
    }

    #[test]
    fn parses_emphasis_inline_markup_in_paragraphs() {
        let document = parse_jira("This is _italic_ text").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Paragraph {
                    content: vec![
                        Inline::Text("This is ".to_string()),
                        Inline::Emphasis(raw_text("italic")),
                        Inline::Text(" text".to_string()),
                    ],
                }],
            }
        );
    }

    #[test]
    fn parses_nested_inline_markup() {
        let document = parse_jira("h2. *strong +inserted+*").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Heading {
                    level: 2,
                    content: vec![Inline::Strong(vec![
                        Inline::Text("strong ".to_string()),
                        Inline::Inserted(raw_text("inserted")),
                    ])],
                }],
            }
        );
    }

    #[test]
    fn parses_nested_inline_markup_inside_emphasis() {
        let document = parse_jira("_very *important*_").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Paragraph {
                    content: vec![Inline::Emphasis(vec![
                        Inline::Text("very ".to_string()),
                        Inline::Strong(raw_text("important")),
                    ])],
                }],
            }
        );
    }

    #[test]
    fn parses_color_and_links() {
        let document = parse_jira(
            "{color:red}red *strong*{color} [http://jira.atlassian.com] [Atlassian|https://atlassian.com]",
        )
        .unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Paragraph {
                    content: vec![
                        Inline::Color {
                            color: "red".to_string(),
                            content: vec![
                                Inline::Text("red ".to_string()),
                                Inline::Strong(raw_text("strong")),
                            ],
                        },
                        Inline::Text(" ".to_string()),
                        Inline::Link {
                            text: None,
                            url: "http://jira.atlassian.com".to_string(),
                        },
                        Inline::Text(" ".to_string()),
                        Inline::Link {
                            text: Some(raw_text("Atlassian")),
                            url: "https://atlassian.com".to_string(),
                        },
                    ],
                }],
            }
        );
    }

    #[test]
    fn parses_inline_markup_in_list_item_paragraphs() {
        let document =
            parse_jira("- item with -gone- and [Atlassian|http://atlassian.com]").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::UnorderedList {
                    items: vec![ListItem {
                        blocks: vec![Block::Paragraph {
                            content: vec![
                                Inline::Text("item with ".to_string()),
                                Inline::Strikethrough(raw_text("gone")),
                                Inline::Text(" and ".to_string()),
                                Inline::Link {
                                    text: Some(raw_text("Atlassian")),
                                    url: "http://atlassian.com".to_string(),
                                },
                            ],
                        }],
                    }],
                }],
            }
        );
    }

    #[test]
    fn leaves_unclosed_or_invalid_inline_markup_as_text() {
        let document =
            parse_jira("*open [label|ftp://example.com] {color:red}missing close").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Paragraph {
                    content: raw_text("*open [label|ftp://example.com] {color:red}missing close"),
                }],
            }
        );
    }

    #[test]
    fn leaves_unclosed_emphasis_markup_as_text() {
        let document = parse_jira("_italic").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![Block::Paragraph {
                    content: raw_text("_italic"),
                }],
            }
        );
    }

    #[test]
    fn preserves_emphasis_markup_inside_code_and_noformat_blocks() {
        let document =
            parse_jira("{code}\n_italic_\n{code}\n{noformat}\n_italic_\n{noformat}").unwrap();

        assert_eq!(
            document,
            Document {
                blocks: vec![
                    Block::CodeBlock {
                        text: "_italic_".to_string(),
                    },
                    Block::NoFormatBlock {
                        text: "_italic_".to_string(),
                    },
                ],
            }
        );
    }
}
