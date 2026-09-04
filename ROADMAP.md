# Jira Markup Parser Roadmap

## Ultimate Goal

Build a bidirectional converter between Jira 7.x wiki markup and Markdown.

The conversion pipeline must always go through an AST:

```text
Jira markup -> Jira AST -> Markdown
Markdown -> Markdown AST or common AST -> Jira markup
```

The first milestone focuses only on parsing Jira markup into a Jira-specific AST. Rendering to Markdown is intentionally out of scope until the parser has a stable shape.

## First Small Goal

Implement a minimal Jira 7.x block-level parser that can parse the following constructs into an AST:

- Headings: `h1.` through `h6.`
- Paragraphs
- Ordered list items using `#`
- Unordered list items using `-`
- Fenced Jira code blocks using `{code}` ... `{code}`
- Blank lines as block separators

This goal should support the example:

```jira
h1. Level 1 Title
This is an ordered list:
# first
# second

This is an unordered list:
- apple
- banana

This is a code block
{code}
const obj = {};
{code}
```

## Non-Goals For The First Small Goal

Do not implement these yet:

- Markdown output
- Markdown-to-Jira conversion
- Inline formatting such as `*strong*`, `_emphasis_`, `{{monospace}}`, links, mentions, colors, or citations
- Nested lists
- List continuation paragraphs
- Tables
- Panels, quotes, macros other than `{code}`
- Code block attributes such as language names or titles
- Full Jira escaping rules
- Error recovery beyond basic unclosed code block detection

## Suggested Crate Shape

Start with a library crate so parser behavior is easy to test.

```text
src/
  lib.rs
  ast.rs
  parser.rs
tests/
  block_parser.rs
```

Suggested public API:

```rust
pub fn parse_jira(input: &str) -> Result<Document, ParseError>;
```

Suggested initial AST:

```rust
pub struct Document {
    pub blocks: Vec<Block>,
}

pub enum Block {
    Heading { level: u8, text: String },
    Paragraph { text: String },
    OrderedList { items: Vec<ListItem> },
    UnorderedList { items: Vec<ListItem> },
    CodeBlock { text: String },
}

pub struct ListItem {
    pub blocks: Vec<Block>,
}
```

For the first milestone, each list item may contain only one paragraph block. Keeping `blocks` now leaves room for nested content later without changing the public AST shape immediately.

## Parsing Rules For The First Goal

### Document

- Parse input line by line.
- Normalize line endings from `\r\n` and `\r` to `\n`.
- Treat one or more blank lines as block separators.
- Preserve source order of all parsed blocks.

### Headings

- Match only lines beginning with `h1. `, `h2. `, ..., `h6. `.
- Parse the heading level from the digit.
- Store the rest of the line as plain text.
- Do not parse inline markup inside the heading yet.

### Paragraphs

- A paragraph starts when a nonblank line does not match another block construct.
- Consecutive paragraph lines are joined with `\n` or a single space; choose one behavior and test it.
- A paragraph ends before a blank line or another block-starting line.

Recommended first behavior: join consecutive paragraph lines with `\n` to preserve source information.

### Ordered Lists

- Match consecutive lines beginning with `# `.
- Each matched line becomes one `ListItem`.
- Strip the marker and store the remaining text as a paragraph inside the item.
- Stop the list at the first non-list line.

### Unordered Lists

- Match consecutive lines beginning with `- `.
- Each matched line becomes one `ListItem`.
- Strip the marker and store the remaining text as a paragraph inside the item.
- Stop the list at the first non-list line.

### Code Blocks

- Match a line exactly equal to `{code}` after trimming surrounding whitespace.
- Collect all following lines verbatim until another trimmed `{code}` line.
- Store the collected content without the closing `{code}` line.
- Preserve newlines inside the code block.
- Return a parse error for an unclosed code block.

## Implementation Steps

1. Create a Rust library crate with `cargo init --lib` if the project is still empty.
2. Define the AST types in `src/ast.rs`.
3. Define `ParseError` and `parse_jira` in `src/parser.rs`.
4. Implement a simple line-oriented parser.
5. Add unit tests for each block type.
6. Add an integration test for the full sample input.
7. Run `cargo fmt` and `cargo test`.
8. Only after the parser tests are stable, start a second roadmap for Markdown rendering.

## Acceptance Criteria

The first small goal is complete when:

- `parse_jira` returns a `Document` AST for the sample Jira markup.
- The sample AST contains:
  - one level-1 heading
  - one paragraph before the ordered list
  - one ordered list with two items
  - one paragraph before the unordered list
  - one unordered list with two items
  - one paragraph before the code block
  - one code block containing `const obj = {};`
- Unclosed `{code}` blocks return an error.
- All parser tests pass with `cargo test`.

## Next Milestone Preview

After this first parser milestone, the next useful goal is Markdown rendering from this limited AST:

```text
Jira markup -> Jira AST -> Markdown
```

That milestone should not add new Jira syntax. It should prove that the AST is useful before expanding the parser surface.
