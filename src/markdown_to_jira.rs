use markdown::mdast::{AlignKind, Image, Link, List, ListItem, Node, Table, TableCell, TableRow};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MarkdownToJiraOptions {}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MarkdownToJiraOutput {
    pub markup: String,
    pub warnings: Vec<MarkdownToJiraWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownToJiraWarning {
    pub kind: MarkdownToJiraWarningKind,
    pub message: String,
    pub lossy: bool,
    pub source_excerpt: Option<String>,
    pub output_excerpt: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkdownToJiraWarningKind {
    ImageAltTextDropped,
    ImageTitleDropped,
    InlineStyleBoundary,
    ListContinuation,
    LinkTitleDropped,
    RawHtmlPreserved,
    TableAlignmentDropped,
    TaskListCheckboxText,
    UnsupportedCodeLanguage,
    UnsupportedMarkdownNode,
    UnsupportedListChild,
    UnsupportedTableChild,
    UnsupportedTableRowChild,
}

impl MarkdownToJiraWarningKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ImageAltTextDropped => "image-alt-text-dropped",
            Self::ImageTitleDropped => "image-title-dropped",
            Self::InlineStyleBoundary => "inline-style-boundary",
            Self::ListContinuation => "list-continuation",
            Self::LinkTitleDropped => "link-title-dropped",
            Self::RawHtmlPreserved => "raw-html-preserved",
            Self::TableAlignmentDropped => "table-alignment-dropped",
            Self::TaskListCheckboxText => "task-list-checkbox-text",
            Self::UnsupportedCodeLanguage => "unsupported-code-language",
            Self::UnsupportedMarkdownNode => "unsupported-markdown-node",
            Self::UnsupportedListChild => "unsupported-list-child",
            Self::UnsupportedTableChild => "unsupported-table-child",
            Self::UnsupportedTableRowChild => "unsupported-table-row-child",
        }
    }
}

impl std::fmt::Display for MarkdownToJiraWarningKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownToJiraError {
    pub message: String,
}

pub fn markdown_to_jira(markdown: &str) -> Result<MarkdownToJiraOutput, MarkdownToJiraError> {
    markdown_to_jira_with_options(markdown, MarkdownToJiraOptions::default())
}

pub fn markdown_to_jira_with_options(
    markdown: &str,
    _options: MarkdownToJiraOptions,
) -> Result<MarkdownToJiraOutput, MarkdownToJiraError> {
    let ast = markdown::to_mdast(markdown, &markdown::ParseOptions::gfm()).map_err(|error| {
        MarkdownToJiraError {
            message: error.to_string(),
        }
    })?;

    let mut renderer = Renderer::new(markdown);
    let markup = renderer.render_document(&ast).trim().to_string();

    Ok(MarkdownToJiraOutput {
        markup,
        warnings: renderer.warnings,
    })
}

struct Renderer<'a> {
    source: &'a str,
    warnings: Vec<MarkdownToJiraWarning>,
}

#[derive(Debug, Clone, Copy, Default)]
struct InlineRenderContext {
    in_table_cell: bool,
}

struct InlineRenderedPart<'a> {
    node: &'a Node,
    markup: String,
}

impl<'a> Renderer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            warnings: Vec::new(),
        }
    }

    fn render_document(&mut self, node: &Node) -> String {
        match node {
            Node::Root(root) => self.render_blocks(&root.children),
            _ => self.render_block(node, &mut Vec::new()),
        }
    }

    fn render_blocks(&mut self, children: &[Node]) -> String {
        children
            .iter()
            .filter_map(|child| {
                let rendered = self.render_block(child, &mut Vec::new());
                (!rendered.is_empty()).then_some(rendered)
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    fn render_block(&mut self, node: &Node, list_prefix: &mut Vec<char>) -> String {
        match node {
            Node::Root(root) => self.render_blocks(&root.children),
            Node::Paragraph(paragraph) => self.render_inlines(&paragraph.children),
            Node::Heading(heading) => {
                format!(
                    "h{}. {}",
                    heading.depth,
                    self.render_inlines(&heading.children)
                )
            }
            Node::Code(code) => self.render_code_block(code.lang.as_deref(), &code.value),
            Node::Blockquote(blockquote) => {
                let body = self.render_blocks(&blockquote.children);
                body.lines()
                    .map(|line| format!("bq. {line}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            Node::List(list) => self.render_list(list, list_prefix),
            Node::Table(table) => self.render_table(table),
            Node::ThematicBreak(_) => "----".to_string(),
            Node::Break(_) => "\n".to_string(),
            Node::Html(html) => self.render_html(&html.value),
            Node::Text(_)
            | Node::Emphasis(_)
            | Node::Strong(_)
            | Node::Delete(_)
            | Node::InlineCode(_)
            | Node::Link(_)
            | Node::Image(_) => self.render_inline(node),
            _ => self.render_unsupported(node),
        }
    }

    fn render_list(&mut self, list: &List, list_prefix: &mut Vec<char>) -> String {
        let marker = if list.ordered { '#' } else { '*' };
        list_prefix.push(marker);

        let rendered = list
            .children
            .iter()
            .filter_map(|child| match child {
                Node::ListItem(item) => {
                    let rendered = self.render_list_item(item, list_prefix);
                    (!rendered.is_empty()).then_some(rendered)
                }
                other => {
                    self.warn(
                        MarkdownToJiraWarningKind::UnsupportedListChild,
                        "Unsupported non-list-item child in list; flattened.",
                    );
                    let rendered = self.render_block(other, list_prefix);
                    (!rendered.is_empty()).then_some(rendered)
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        list_prefix.pop();
        rendered
    }

    fn render_list_item(&mut self, item: &ListItem, list_prefix: &mut Vec<char>) -> String {
        let prefix = format!("{} ", list_prefix.iter().collect::<String>());
        let checkbox = match item.checked {
            Some(true) => {
                self.warn(
                    MarkdownToJiraWarningKind::TaskListCheckboxText,
                    "Task list checkbox state preserved as text; task-list semantics are lossy.",
                );
                "\\[x\\] "
            }
            Some(false) => {
                self.warn(
                    MarkdownToJiraWarningKind::TaskListCheckboxText,
                    "Task list checkbox state preserved as text; task-list semantics are lossy.",
                );
                "\\[ \\] "
            }
            None => "",
        };
        let mut lines = Vec::new();
        let mut has_lossy_continuation = false;

        for (index, child) in item.children.iter().enumerate() {
            match child {
                Node::Paragraph(paragraph) if lines.is_empty() => {
                    let item_text = self.render_inlines(&paragraph.children);
                    lines.push(format!("{prefix}{checkbox}{item_text}"));
                }
                Node::Paragraph(paragraph) => {
                    let item_text = self.render_inlines(&paragraph.children);
                    if !item_text.is_empty() {
                        if index > 0 {
                            has_lossy_continuation = true;
                        }
                        lines.push(item_text);
                    }
                }
                Node::List(list) => {
                    let nested = self.render_list(list, list_prefix);
                    if !nested.is_empty() {
                        lines.push(nested);
                    }
                }
                Node::Code(code) if !lines.is_empty() => {
                    let item_text = self.render_code_block(code.lang.as_deref(), &code.value);
                    if !item_text.is_empty() {
                        if index > 0 {
                            has_lossy_continuation = true;
                        }
                        lines.push(item_text);
                    }
                }
                other if lines.is_empty() => {
                    let item_text = self.render_block(other, list_prefix);
                    lines.push(format!("{prefix}{checkbox}{item_text}"));
                }
                other => {
                    let item_text = self.render_block(other, list_prefix);
                    if !item_text.is_empty() {
                        if index > 0 {
                            has_lossy_continuation = true;
                        }
                        lines.push(item_text);
                    }
                }
            }
        }

        if lines.is_empty() && !checkbox.is_empty() {
            lines.push(format!("{prefix}{}", checkbox.trim_end()));
        }

        let rendered = lines.join("\n");
        if has_lossy_continuation {
            self.warn_with_excerpts(
                MarkdownToJiraWarningKind::ListContinuation,
                "Markdown list item continuation content was emitted without a Jira list marker and may not round-trip as part of the item.",
                true,
                None,
                Some(compact_excerpt(&rendered)),
            );
        }

        rendered
    }

    fn render_table(&mut self, table: &Table) -> String {
        if table.align.iter().any(|align| *align != AlignKind::None) {
            self.warn(
                MarkdownToJiraWarningKind::TableAlignmentDropped,
                "Table alignment metadata dropped; Jira table markup does not support Markdown column alignment.",
            );
        }

        table
            .children
            .iter()
            .enumerate()
            .filter_map(|(index, child)| match child {
                Node::TableRow(row) => Some(self.render_table_row(row, index == 0)),
                other => {
                    self.warn(
                        MarkdownToJiraWarningKind::UnsupportedTableChild,
                        "Unsupported non-row child in table; flattened.",
                    );
                    let rendered = self.render_inline(other);
                    (!rendered.is_empty()).then_some(rendered)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render_table_row(&mut self, row: &TableRow, is_header: bool) -> String {
        let cells = row
            .children
            .iter()
            .filter_map(|child| match child {
                Node::TableCell(cell) => Some(self.render_table_cell(cell)),
                other => {
                    self.warn(
                        MarkdownToJiraWarningKind::UnsupportedTableRowChild,
                        "Unsupported non-cell child in table row; flattened.",
                    );
                    let rendered = self.render_inline(other);
                    (!rendered.is_empty()).then_some(rendered)
                }
            })
            .collect::<Vec<_>>();

        if is_header {
            format!("||{}||", cells.join("||"))
        } else {
            format!("|{}|", cells.join("|"))
        }
    }

    fn render_table_cell(&mut self, cell: &TableCell) -> String {
        self.render_inlines_with_context(
            &cell.children,
            InlineRenderContext {
                in_table_cell: true,
            },
        )
    }

    fn render_inlines(&mut self, children: &[Node]) -> String {
        self.render_inlines_with_context(children, InlineRenderContext::default())
    }

    fn render_inlines_with_context(
        &mut self,
        children: &[Node],
        context: InlineRenderContext,
    ) -> String {
        let mut parts = Vec::with_capacity(children.len());

        for child in children {
            parts.push(InlineRenderedPart {
                node: child,
                markup: self.render_inline_with_context(child, context),
            });
        }

        self.join_inline_parts_with_jira_boundaries(&parts)
    }

    fn render_inline(&mut self, node: &Node) -> String {
        self.render_inline_with_context(node, InlineRenderContext::default())
    }

    fn render_inline_with_context(&mut self, node: &Node, context: InlineRenderContext) -> String {
        match node {
            Node::Text(text) => escape_jira_text(&text.value),
            Node::Emphasis(emphasis) => {
                format!(
                    "_{}_",
                    self.render_inlines_with_context(&emphasis.children, context)
                )
            }
            Node::Strong(strong) => {
                format!(
                    "*{}*",
                    self.render_inlines_with_context(&strong.children, context)
                )
            }
            Node::Delete(delete) => {
                format!(
                    "-{}-",
                    self.render_inlines_with_context(&delete.children, context)
                )
            }
            Node::InlineCode(code) => {
                format!("{{{{{}}}}}", escape_inline_code(&code.value, context))
            }
            Node::Code(code) => self.render_code_block(code.lang.as_deref(), &code.value),
            Node::Link(link) => self.render_link(link, context),
            Node::Image(image) => self.render_image(image),
            Node::Html(html) => self.render_html(&html.value),
            Node::Break(_) => "\n".to_string(),
            Node::Paragraph(paragraph) => {
                self.render_inlines_with_context(&paragraph.children, context)
            }
            Node::Heading(heading) => self.render_inlines_with_context(&heading.children, context),
            Node::Root(root) => self.render_inlines_with_context(&root.children, context),
            other => self.render_unsupported(other),
        }
    }

    fn render_link(&mut self, link: &Link, context: InlineRenderContext) -> String {
        if link.title.is_some() {
            self.warn(
                MarkdownToJiraWarningKind::LinkTitleDropped,
                "Link title dropped; Jira link markup does not preserve Markdown titles.",
            );
        }

        let label = self.render_inlines_with_context(&link.children, context);
        let url = escape_jira_link_url(&link.url);
        if label == url {
            format!("[{url}]")
        } else {
            format!("[{label}|{url}]")
        }
    }

    fn render_image(&mut self, image: &Image) -> String {
        if !image.alt.is_empty() {
            self.warn(
                MarkdownToJiraWarningKind::ImageAltTextDropped,
                "Image alt text dropped; Jira image markup does not preserve Markdown alt text.",
            );
        }
        if image.title.is_some() {
            self.warn(
                MarkdownToJiraWarningKind::ImageTitleDropped,
                "Image title dropped; Jira image markup does not preserve Markdown titles.",
            );
        }

        format!("!{}!", image.url)
    }

    fn render_html(&mut self, value: &str) -> String {
        self.warn(
            MarkdownToJiraWarningKind::RawHtmlPreserved,
            "Raw HTML preserved as text; HTML rendering semantics are lossy.",
        );
        value.to_string()
    }

    fn render_code_block(&mut self, lang: Option<&str>, value: &str) -> String {
        match lang {
            Some(lang) if !lang.trim().is_empty() => {
                let lang = lang.trim().to_ascii_lowercase();

                if is_supported_jira_code_language(&lang) {
                    format!("{{code:{lang}}}\n{value}\n{{code}}")
                } else {
                    let rendered = format!("{{noformat}}\n{value}\n{{noformat}}");
                    self.warn_with_excerpts(
                        MarkdownToJiraWarningKind::UnsupportedCodeLanguage,
                        format!(
                            "Unsupported code block language `{lang}` rendered as `{{noformat}}` for element `code`; lossy true."
                        ),
                        true,
                        None,
                        Some(compact_excerpt(&rendered)),
                    );
                    rendered
                }
            }
            _ => format!("{{code}}\n{value}\n{{code}}"),
        }
    }

    fn render_unsupported(&mut self, node: &Node) -> String {
        self.warn(
            MarkdownToJiraWarningKind::UnsupportedMarkdownNode,
            "Unsupported Markdown node flattened lossy.",
        );

        if let Some(children) = node.children() {
            return self.render_inlines(children);
        }

        match node {
            Node::Image(image) => image.alt.clone(),
            Node::ImageReference(image) => image.alt.clone(),
            _ => node.to_string(),
        }
    }

    fn join_inline_parts_with_jira_boundaries(
        &mut self,
        parts: &[InlineRenderedPart<'_>],
    ) -> String {
        let mut insert_before = vec![false; parts.len()];

        for index in 1..parts.len() {
            if should_insert_inline_boundary_space(&parts[index - 1], &parts[index]) {
                insert_before[index] = true;
            }
        }

        for index in 0..parts.len() {
            if !is_jira_inline_style_node(parts[index].node) {
                continue;
            }

            let left_inserted = index > 0 && insert_before[index];
            let right_inserted = index + 1 < parts.len() && insert_before[index + 1];

            if left_inserted || right_inserted {
                let first = if left_inserted {
                    parts[index - 1].node
                } else {
                    parts[index].node
                };
                let last = if right_inserted {
                    parts[index + 1].node
                } else {
                    parts[index].node
                };

                self.warn_with_excerpts(
                    MarkdownToJiraWarningKind::InlineStyleBoundary,
                    "Inserted spaces around Jira inline style to preserve rendering.",
                    false,
                    self.source_excerpt(first, last),
                    Some(compact_excerpt(&inline_output_excerpt(
                        parts,
                        index,
                        left_inserted,
                        right_inserted,
                    ))),
                );
            }
        }

        let mut rendered = String::new();
        for (index, part) in parts.iter().enumerate() {
            if insert_before[index] {
                rendered.push(' ');
            }
            rendered.push_str(&part.markup);
        }

        rendered
    }

    fn source_excerpt(&self, first: &Node, last: &Node) -> Option<String> {
        let start = first.position()?.start.offset;
        let end = last.position()?.end.offset;
        let start = source_offset_to_byte_index(self.source, start)?;
        let end = source_offset_to_byte_index(self.source, end)?;

        (start <= end && end <= self.source.len())
            .then(|| compact_excerpt(&self.source[start..end]))
    }

    fn warn(&mut self, kind: MarkdownToJiraWarningKind, message: impl Into<String>) {
        self.warn_with_excerpts(kind, message, true, None, None);
    }

    fn warn_with_excerpts(
        &mut self,
        kind: MarkdownToJiraWarningKind,
        message: impl Into<String>,
        lossy: bool,
        source_excerpt: Option<String>,
        output_excerpt: Option<String>,
    ) {
        self.warnings.push(MarkdownToJiraWarning {
            kind,
            message: message.into(),
            lossy,
            source_excerpt,
            output_excerpt,
        });
    }
}

fn inline_output_excerpt(
    parts: &[InlineRenderedPart<'_>],
    index: usize,
    left_inserted: bool,
    right_inserted: bool,
) -> String {
    let mut excerpt = String::new();

    if left_inserted {
        excerpt.push_str(&parts[index - 1].markup);
        excerpt.push(' ');
    }
    excerpt.push_str(&parts[index].markup);
    if right_inserted {
        excerpt.push(' ');
        excerpt.push_str(&parts[index + 1].markup);
    }

    excerpt
}

fn should_insert_inline_boundary_space(
    left: &InlineRenderedPart<'_>,
    right: &InlineRenderedPart<'_>,
) -> bool {
    (is_jira_inline_style_node(left.node) && starts_with_risky_text(right))
        || (is_jira_inline_style_node(right.node) && ends_with_risky_text(left))
}

fn starts_with_risky_text(part: &InlineRenderedPart<'_>) -> bool {
    is_text_node(part.node)
        && part
            .markup
            .chars()
            .next()
            .is_some_and(is_risky_inline_boundary_text)
}

fn ends_with_risky_text(part: &InlineRenderedPart<'_>) -> bool {
    is_text_node(part.node)
        && part
            .markup
            .chars()
            .next_back()
            .is_some_and(is_risky_inline_boundary_text)
}

fn is_text_node(node: &Node) -> bool {
    matches!(node, Node::Text(_))
}

fn is_jira_inline_style_node(node: &Node) -> bool {
    matches!(
        node,
        Node::Strong(_) | Node::Emphasis(_) | Node::Delete(_) | Node::InlineCode(_)
    )
}

fn is_risky_inline_boundary_text(character: char) -> bool {
    character.is_alphanumeric() || !character.is_ascii()
}

fn source_offset_to_byte_index(source: &str, offset: usize) -> Option<usize> {
    if offset <= source.len() && source.is_char_boundary(offset) {
        return Some(offset);
    }

    if offset == source.chars().count() {
        return Some(source.len());
    }

    source.char_indices().nth(offset).map(|(index, _)| index)
}

fn compact_excerpt(value: &str) -> String {
    const MAX_CHARS: usize = 80;

    let escaped = value.replace('\n', "\\n");
    if escaped.chars().count() <= MAX_CHARS {
        return escaped;
    }

    let mut truncated = escaped.chars().take(MAX_CHARS - 3).collect::<String>();
    truncated.push_str("...");
    truncated
}

fn is_supported_jira_code_language(lang: &str) -> bool {
    matches!(
        lang,
        "actionscript" | "html" | "java" | "javascript" | "none" | "sql" | "xhtml" | "xml"
    )
}

fn escape_jira_text(value: &str) -> String {
    escape_chars(value, |character| {
        matches!(
            character,
            '*' | '_' | '-' | '+' | '^' | '~' | '[' | ']' | '{' | '}' | '!' | '|'
        )
    })
}

fn escape_inline_code(value: &str, context: InlineRenderContext) -> String {
    escape_chars(value, |character| {
        matches!(character, '{' | '}') || (context.in_table_cell && character == '|')
    })
}

fn escape_jira_link_url(value: &str) -> String {
    escape_chars(value, |character| matches!(character, '|' | ']'))
}

fn escape_chars(value: &str, should_escape: impl Fn(char) -> bool) -> String {
    let mut escaped = String::with_capacity(value.len());

    for character in value.chars() {
        if should_escape(character) {
            escaped.push('\\');
        }
        escaped.push(character);
    }

    escaped
}
