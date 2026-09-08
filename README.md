# jira-parser

`jira-parser` is an experimental Rust parser and converter targeting Jira 7.x wiki markup.

The `v0.1.0` preview is useful for testing and integration, but it is not a complete Jira wiki implementation. Syntax coverage, AST types, conversion behavior, and CLI output may change before `1.0.0`.

## Current status

The project currently provides:

- Jira markup to a typed Rust AST.
- Markdown to Jira 7.x wiki markup, parsed through a Markdown AST first.
- A CLI for both operations.
- A Rust library API for parsing and conversion.

Jira-to-Markdown conversion is the intended next direction, but it is **not implemented** in this preview.

## Installation

### Release binaries

When binaries are published, download the archive for Windows x86-64, Linux x86-64, or macOS Apple Silicon from the repository's GitHub Releases page.

This crate is not documented as a crates.io installation. Build it from source when a release binary is not available.

### Build from source

Install the stable Rust toolchain, clone the repository, and run:

```console
cargo build --locked --release
```

The executable is written to `target/release/jira-parser` (`target/release/jira-parser.exe` on Windows).

## CLI

```text
Usage: jira-parser [--markdown-to-jira] --text <TEXT> [--output <PATH|->]
       jira-parser [--markdown-to-jira] -t <TEXT> [-o <PATH|->]
       jira-parser [--markdown-to-jira] --file <PATH> [--output <PATH|->]
       jira-parser [--markdown-to-jira] -f <PATH> [-o <PATH|->]
       jira-parser --version
       jira-parser -v
```

Exactly one input source is required for parsing or conversion:

- `--text`, `-t`: read the input from the following argument.
- `--file`, `-f`: read UTF-8 text from a file.

Version options:

- `--version`, `-v`: print the installed `jira-parser` version without requiring input. Version options cannot be combined with any other option.

Output options:

- `--output <PATH>`, `-o <PATH>`: write the operation result to a file.
- `--output -`, `-o -`: write the result to stdout.
- If `--output` is omitted, stdout is used.

Operation results contain only the formatted AST or converted Jira markup. Conversion warnings and CLI errors are written to stderr, so stdout can be redirected or piped safely.

### Parse Jira markup to AST

Jira parsing is the default mode:

```console
jira-parser -t "h1. Preview"
```

Example output:

```text
Document {
    blocks: [
        Heading {
            level: 1,
            content: [
                Text(
                    "Preview",
                ),
            ],
        },
    ],
}
```

Parse a file and store the AST:

```console
jira-parser --file example.jira --output ast.txt
```

### Convert Markdown to Jira markup

Use `--markdown-to-jira` to select conversion mode:

```console
jira-parser --markdown-to-jira -t "# Changes" -o -
```

Equivalent output:

```text
h1. Changes
```

Convert between files while keeping warnings on the console:

```console
jira-parser --markdown-to-jira --file notes.md --output notes.jira
```

## Jira parser coverage

The Jira-to-AST parser currently recognizes:

- Headings `h1. ` through `h6. `.
- Single-line and multi-line paragraphs.
- Ordered lists using `# ` and unordered lists using `- `.
- Nested and mixed lists using repeated markers, such as `## ` and `-# `.
- `{code}` and `{noformat}` blocks.
- Line quotes using `bq. ` and fenced `{quote}` blocks.
- Header rows using `||cell||` and body rows using `|cell|`.
- Inline bold (`*text*`), italic (`_text_`), insertion/underline (`+text+`), and strikethrough (`-text-`).
- Text color using `{color:name}text{color}`.
- HTTP and HTTPS links using `[URL]` or `[label|URL]`.

Leading spaces and tabs are stripped from normal lines before parsing. Content inside `{code}` and `{noformat}` blocks preserves its original whitespace. Unclosed code, noformat, and quote blocks return parse errors.

Inline Jira styling is based on observed Jira 7.x behavior and remains heuristic for ambiguous or deeply nested delimiter sequences.

## Markdown conversion coverage

Markdown input is parsed with the `markdown` crate's GFM mode and then rendered as Jira wiki markup. Implemented mappings include:

| Markdown | Jira 7.x wiki markup |
| --- | --- |
| Headings 1-6 | `h1. ` through `h6. ` |
| Paragraphs | Plain text separated by blank lines |
| Emphasis, strong, strikethrough | `_text_`, `*text*`, `-text-` |
| Inline code | `{{code}}` |
| Ordered and unordered lists | `# ` and `* `, including nesting |
| Fenced and indented code | `{code}` blocks |
| Blockquotes | `bq. ` lines |
| GFM tables | Jira header and body rows |
| Links | `[label|URL]` or `[URL]` |
| Images | `!URL!` |
| Thematic breaks | `----` |
| Hard line breaks | Line breaks |

Code block languages are emitted only for `actionscript`, `html`, `java`, `javascript`, `none`, `sql`, `xhtml`, and `xml`. Other language tags are rendered with `{noformat}` and produce a warning because unsupported Jira code languages can render incorrectly.

The converter escapes Jira control characters in literal text and may insert spaces around inline styles so Jira 7.x renders them. A warning includes source and output excerpts when this boundary adjustment occurs.

### Lossy and unsupported Markdown

Some Markdown structures have no reliable Jira 7.x wiki equivalent. Current fallback behavior includes:

- Image alt text and image/link titles are dropped with warnings.
- Table alignment is dropped with a warning.
- Task-list state is retained as escaped text such as `\[x\]`, with a warning.
- Raw HTML is preserved as text, with a warning; HTML rendering semantics are not guaranteed.
- Unsupported code languages use `{noformat}`.
- Continuation blocks inside list items are emitted without an extra list marker and warn because Jira may not retain their association with the item.
- Other unsupported Markdown AST nodes are flattened where possible and warn that conversion is lossy.

Review stderr warnings before publishing converted text to Jira. See [the conversion guidelines](docs/markdown-jira-conversion-guidelines.md) for the intended mapping and fallback policy.

## Known limitations

- Jira-to-Markdown conversion is not implemented.
- The Jira parser covers the syntax listed above, not the complete Jira wiki renderer grammar. For example, parameterized Jira code macros such as `{code:java}` are not parsed as code blocks yet.
- Complex nested Jira inline styles are heuristic and may differ from a particular Jira installation.
- Markdown list-item continuation blocks have no dependable Jira 7.x representation and can lose their list association.
- Markdown extensions without a direct mapping may be flattened or preserved as text.
- This project targets Jira 7.x wiki markup, not Atlassian Document Format (ADF). Renderer behavior can also vary between Jira installations and plugins.
- The CLI contract, public AST, warning types, and conversion details may change before `1.0.0`.

## Library API

The crate exports the AST types and these primary operations:

```rust
use jira_parser::{markdown_to_jira, parse_jira};

let document = parse_jira("h1. Preview").expect("valid Jira markup");
let converted = markdown_to_jira("# Preview").expect("valid Markdown");

println!("{document:#?}");
println!("{}", converted.markup);
for warning in converted.warnings {
    eprintln!("warning[{}]: {}", warning.kind, warning.message);
}
```

`markdown_to_jira_with_options` is also public, although `MarkdownToJiraOptions` has no configurable fields in `v0.1.0`. Treat public AST and conversion types as unstable until `1.0.0`.

## Development

Format, check, and test the project with:

```console
cargo fmt --check
cargo check --locked
cargo test --locked
```

Create an optimized local build with:

```console
cargo build --locked --release
```

## License

This project is licensed under the GNU General Public License version 2. See [LICENSE](LICENSE).
