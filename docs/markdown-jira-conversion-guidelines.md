# Markdown to Jira Conversion Guidelines

## Scope

These guidelines cover conversion from Markdown into Jira 7.x wiki syntax.
They are not for Atlassian Document Format (ADF), which is used by newer
Jira Cloud APIs and has a different structured JSON representation.

The converter should prefer predictable, implementation-oriented behavior over
visual approximations. Use the Markdown AST as the source of truth, emit Jira
wiki syntax where there is a clear mapping, and collect warnings whenever a
fallback is lossy or only partially preserves intent.

## Markdown to Jira Element Mapping

| Markdown element | Jira 7.x wiki syntax | Notes |
| --- | --- | --- |
| Paragraph | Plain text separated by blank lines | Preserve paragraph boundaries. |
| Heading 1-6 | `h1. Text` through `h6. Text` | Preserve heading level when possible. |
| Emphasis | `_text_` | Applies to inline emphasis. |
| Strong | `*text*` | Applies to inline strong text. |
| Strikethrough | `-text-` | Only map when the Markdown parser exposes it. |
| Inline code | `{{code}}` | Escape or normalize braces as needed. |
| Fenced code block | `{code[:language]}...{code}` | Include language when available and supported. |
| Blockquote | `bq. text` | Multi-paragraph blockquotes may need repeated `bq.` lines. |
| Unordered list | `* item` | Nest with additional `*` markers. |
| Ordered list | `# item` | Nest with additional `#` markers. |
| Link | `[text\|url]` | Preserve link text and target. |
| Image | `!url!` | Alt text has no direct Jira wiki equivalent. |
| Table | `|| header ||` and `| cell |` rows | Requires a simple rectangular table. |
| Horizontal rule | `----` | Treat as a block separator. |
| Hard line break | Line break | Preserve where semantically meaningful. |

## Unsupported or Poorly Mapped Markdown Elements

Some Markdown elements do not have a reliable Jira 7.x wiki equivalent:

- HTML blocks and inline HTML.
- Footnotes.
- Definition lists.
- Task list checkbox state.
- Nested tables or complex table cell content.
- Table alignment.
- Image alt text and title metadata.
- Link title metadata.
- Attribute extensions, custom IDs, and classes.
- Math blocks or inline math.
- Mermaid diagrams and other fenced diagram syntaxes.
- Admonitions or callouts unless represented by a known extension.

These elements should not be silently dropped. The converter should either
preserve readable text, wrap source content in a safe fallback, or emit a
warning that explains what was lost.

## Strategy for Handling Unsupported Markdown Elements

Use a small set of explicit fallback strategies:

1. Preserve readable text when the element's primary value is textual.
2. Emit the original Markdown as plain text when preserving syntax is more
   useful than approximating display.
3. Convert fenced extension blocks to `{code}` blocks when Jira cannot render
   the extension natively.
4. Flatten unsupported inline formatting while keeping child text.
5. Drop only metadata that has no visible or recoverable Jira representation,
   and record a warning.

Each warning should identify the Markdown element, describe the fallback, and
state whether information was lost. Warnings should be collected during
conversion and returned to the caller instead of being printed directly.

## AST-First Conversion Guidance

Conversion should operate on the parsed Markdown AST, not on raw Markdown text.
AST-first conversion gives the implementation a stable representation of block
and inline structure, avoids fragile regular expressions, and makes warning
collection precise.

Recommended implementation flow:

1. Parse Markdown into an AST using the configured Markdown parser.
2. Walk block nodes and dispatch by node type.
3. For each block node, render child inline nodes through a separate inline
   renderer.
4. Track list depth, table context, blockquote context, and code block language
   explicitly.
5. Escape Jira wiki control characters only at the layer where text is emitted.
6. Append structured warnings for unsupported nodes, lossy metadata, and
   fallback rendering.

Avoid pre-processing Markdown with broad string replacements. If normalization
is required, keep it narrow and document which parser behavior it supports.

## Lossy vs Lossless Expectations

A conversion is lossless when the visible content and structure have a direct
Jira 7.x wiki equivalent: headings, paragraphs, basic inline formatting, simple
lists, links, basic code blocks, simple blockquotes, and rectangular tables.

A conversion is lossy when Jira wiki syntax cannot represent part of the
Markdown source, such as attributes, HTML semantics, task checkbox state,
diagram rendering, table alignment, or image alt text. Lossy conversions are
acceptable when they are deliberate, readable, and reported through warnings.

The expected contract is:

- Preserve readable user content by default.
- Prefer Jira-native syntax when there is a clear mapping.
- Use plain-text or `{code}` fallbacks for unsupported rich structures.
- Return warnings for every lossy fallback.
- Do not claim ADF compatibility from this conversion path.
