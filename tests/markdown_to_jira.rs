use jira_parser::markdown_to_jira;

fn convert(markdown: &str) -> (String, Vec<String>) {
    let output = markdown_to_jira(markdown).unwrap();
    let warnings = output
        .warnings
        .into_iter()
        .map(|warning| warning.message)
        .collect();

    (output.markup, warnings)
}

#[test]
fn converts_heading_and_paragraph() {
    let (markup, warnings) = convert("# Release Notes\n\nThis is the opening paragraph.");

    assert_eq!(
        markup,
        "h1. Release Notes\n\nThis is the opening paragraph."
    );
    assert!(warnings.is_empty());
}

#[test]
fn converts_inline_markup() {
    let (markup, warnings) = convert(
        "This has *emphasis*, **strong**, ~~strike~~, `inline code`, and [a link](https://example.com).",
    );

    assert_eq!(
        markup,
        "This has _emphasis_, *strong*, -strike-, {{inline code}}, and [a link|https://example.com]."
    );
    assert!(warnings.is_empty());
}

#[test]
fn inserts_boundary_spaces_around_glued_strong_text() {
    let output = markdown_to_jira("a**serious**event").unwrap();

    assert_eq!(output.markup, "a *serious* event");
    assert_eq!(output.warnings.len(), 1);

    let warning = &output.warnings[0];
    assert_eq!(warning.kind.as_str(), "inline-style-boundary");
    assert_eq!(
        warning.message,
        "Inserted spaces around Jira inline style to preserve rendering."
    );
    assert!(!warning.lossy);
    assert_eq!(warning.source_excerpt.as_deref(), Some("a**serious**event"));
    assert_eq!(warning.output_excerpt.as_deref(), Some("a *serious* event"));
}

#[test]
fn inserts_boundary_spaces_around_glued_emphasis_text() {
    let output = markdown_to_jira("a*em*event").unwrap();

    assert_eq!(output.markup, "a _em_ event");
    assert_eq!(output.warnings.len(), 1);
    assert_eq!(output.warnings[0].kind.as_str(), "inline-style-boundary");
    assert_eq!(
        output.warnings[0].source_excerpt.as_deref(),
        Some("a*em*event")
    );
    assert_eq!(
        output.warnings[0].output_excerpt.as_deref(),
        Some("a _em_ event")
    );
}

#[test]
fn inserts_boundary_spaces_around_glued_strikethrough_text() {
    let output = markdown_to_jira("a~~old~~event").unwrap();

    assert_eq!(output.markup, "a -old- event");
    assert_eq!(output.warnings.len(), 1);
    assert_eq!(output.warnings[0].kind.as_str(), "inline-style-boundary");
    assert_eq!(
        output.warnings[0].source_excerpt.as_deref(),
        Some("a~~old~~event")
    );
    assert_eq!(
        output.warnings[0].output_excerpt.as_deref(),
        Some("a -old- event")
    );
}

#[test]
fn preserves_existing_spaces_around_strong_text() {
    let (markup, warnings) = convert("a **serious** event");

    assert_eq!(markup, "a *serious* event");
    assert!(warnings.is_empty());
}

#[test]
fn inserts_boundary_spaces_around_cjk_adjacent_strong_text() {
    let output = markdown_to_jira("中**重**文").unwrap();

    assert_eq!(output.markup, "中 *重* 文");
    assert_eq!(output.warnings.len(), 1);
    assert_eq!(output.warnings[0].kind.as_str(), "inline-style-boundary");
    assert_eq!(
        output.warnings[0].source_excerpt.as_deref(),
        Some("中**重**文")
    );
    assert_eq!(
        output.warnings[0].output_excerpt.as_deref(),
        Some("中 *重* 文")
    );
}

#[test]
fn inserts_boundary_spaces_around_glued_inline_code_text() {
    let output = markdown_to_jira("a`code`event").unwrap();

    assert_eq!(output.markup, "a {{code}} event");
    assert_eq!(output.warnings.len(), 1);
    assert_eq!(output.warnings[0].kind.as_str(), "inline-style-boundary");
    assert_eq!(
        output.warnings[0].source_excerpt.as_deref(),
        Some("a`code`event")
    );
    assert_eq!(
        output.warnings[0].output_excerpt.as_deref(),
        Some("a {{code}} event")
    );
}

#[test]
fn escapes_literal_jira_control_characters() {
    let cases = [
        (
            "日志出现 [ERROR] 和 [REDACTED] 标记",
            "日志出现 \\[ERROR\\] 和 \\[REDACTED\\] 标记",
        ),
        ("响应包含 {status} 字段", "响应包含 \\{status\\} 字段"),
        (
            "字段 my_var_name 与 result=S_OK",
            "字段 my\\_var\\_name 与 result=S\\_OK",
        ),
        ("版本 ^1.2.3 与 ~user 路径", "版本 \\^1.2.3 与 \\~user 路径"),
        (
            "符号 * _ - + ^ ~ ! |",
            "符号 \\* \\_ \\- \\+ \\^ \\~ \\! \\|",
        ),
        ("字面量 \\*星号\\* 测试", "字面量 \\*星号\\* 测试"),
    ];

    for (markdown, expected) in cases {
        let (markup, warnings) = convert(markdown);

        assert_eq!(markup, expected);
        assert!(warnings.is_empty());
    }
}

#[test]
fn converts_inline_code() {
    let (markup, warnings) = convert("Use `const obj = {};` inline.");

    assert_eq!(markup, "Use {{const obj = \\{\\};}} inline.");
    assert!(warnings.is_empty());
}

#[test]
fn escapes_braces_inside_inline_code() {
    let (markup, warnings) = convert("调用 `{code}` 宏或 `a{b}c` 时");

    assert_eq!(markup, "调用 {{\\{code\\}}} 宏或 {{a\\{b\\}c}} 时");
    assert!(warnings.is_empty());
}

#[test]
fn converts_fenced_code_without_language() {
    let (markup, warnings) = convert(
        r#"```
const obj = {};
obj['cars'] = ['Ford'];
```"#,
    );

    assert_eq!(
        markup,
        r#"{code}
const obj = {};
obj['cars'] = ['Ford'];
{code}"#
    );
    assert!(warnings.is_empty());
}

#[test]
fn converts_supported_javascript_fenced_code_language() {
    let (markup, warnings) = convert(
        r#"```javascript
const obj = {};
obj['cars'] = ['Ford'];
```"#,
    );

    assert_eq!(
        markup,
        r#"{code:javascript}
const obj = {};
obj['cars'] = ['Ford'];
{code}"#
    );
    assert!(warnings.is_empty());
}

#[test]
fn normalizes_supported_java_fenced_code_language_case() {
    let (markup, warnings) = convert(
        r#"```Java
class Example {}
```"#,
    );

    assert_eq!(
        markup,
        r#"{code:java}
class Example {}
{code}"#
    );
    assert!(warnings.is_empty());
}

#[test]
fn renders_unsupported_bash_fenced_code_language_as_noformat_with_warning() {
    let output = markdown_to_jira(
        r#"```bash
echo "hello"
```"#,
    )
    .unwrap();

    assert_eq!(
        output.markup,
        r#"{noformat}
echo "hello"
{noformat}"#
    );
    assert_eq!(output.warnings.len(), 1);
    assert_eq!(
        output.warnings[0].kind.as_str(),
        "unsupported-code-language"
    );
    assert_eq!(
        output.warnings[0].message,
        "Unsupported code block language `bash` rendered as `{noformat}` for element `code`; lossy true."
    );
    assert!(output.warnings[0].lossy);
    assert_eq!(
        output.warnings[0].output_excerpt.as_deref(),
        Some(r#"{noformat}\necho "hello"\n{noformat}"#)
    );
}

#[test]
fn renders_unsupported_yaml_fenced_code_language_as_noformat_with_warning() {
    let output = markdown_to_jira(
        r#"```yaml
name: service
enabled: true
```"#,
    )
    .unwrap();

    assert_eq!(
        output.markup,
        r#"{noformat}
name: service
enabled: true
{noformat}"#
    );
    assert_eq!(output.warnings.len(), 1);
    assert_eq!(
        output.warnings[0].kind.as_str(),
        "unsupported-code-language"
    );
    assert_eq!(
        output.warnings[0].message,
        "Unsupported code block language `yaml` rendered as `{noformat}` for element `code`; lossy true."
    );
    assert!(output.warnings[0].lossy);
    assert_eq!(
        output.warnings[0].output_excerpt.as_deref(),
        Some(r#"{noformat}\nname: service\nenabled: true\n{noformat}"#)
    );
}

#[test]
fn converts_indented_code_block() {
    let (markup, warnings) = convert("    const obj = {};\n    obj['cars'] = ['Ford'];");

    assert_eq!(
        markup,
        r#"{code}
const obj = {};
obj['cars'] = ['Ford'];
{code}"#
    );
    assert!(warnings.is_empty());
}

#[test]
fn converts_nested_ordered_and_unordered_lists() {
    let (markup, warnings) = convert(
        "1. first\n   - nested bullet\n   - another bullet\n2. second\n   1. nested ordered",
    );

    assert_eq!(
        markup,
        "# first\n#* nested bullet\n#* another bullet\n# second\n## nested ordered"
    );
    assert!(warnings.is_empty());
}

#[test]
fn does_not_warn_for_simple_single_paragraph_list_items() {
    let output = markdown_to_jira("- alpha\n- beta").unwrap();

    assert_eq!(output.markup, "* alpha\n* beta");
    assert!(output.warnings.is_empty());
}

#[test]
fn keeps_fenced_code_block_with_ordered_list_item() {
    let output = markdown_to_jira(
        r#"1. `javascript`
   ```
   const obj = {};
   ```

2. `java`
   ```
   final Map<String, Object> obj = new HashMap<>();
   ```"#,
    )
    .unwrap();

    assert_eq!(
        output.markup,
        r#"# {{javascript}}
{code}
const obj = {};
{code}
# {{java}}
{code}
final Map<String, Object> obj = new HashMap<>();
{code}"#
    );
    assert_eq!(output.warnings.len(), 2);
    assert!(output
        .warnings
        .iter()
        .all(|warning| warning.kind.as_str() == "list-continuation"));
    assert!(output.warnings.iter().all(|warning| warning.lossy));
}

#[test]
fn keeps_continuation_paragraph_after_code_block_in_ordered_list_item() {
    let output = markdown_to_jira(
        r#"1. item 1
   ```
   const obj = {};
   ```
   explanation following the code block
2. item 2"#,
    )
    .unwrap();

    assert_eq!(
        output.markup,
        r#"# item 1
{code}
const obj = {};
{code}
explanation following the code block
# item 2"#
    );
    assert_eq!(output.warnings.len(), 1);
    let warning = &output.warnings[0];
    assert_eq!(warning.kind.as_str(), "list-continuation");
    assert_eq!(
        warning.message,
        "Markdown list item continuation content was emitted without a Jira list marker and may not round-trip as part of the item."
    );
    assert!(warning.lossy);
    assert_eq!(
        warning.output_excerpt.as_deref(),
        Some("# item 1\\n{code}\\nconst obj = {};\\n{code}\\nexplanation following the code block")
    );
    assert_eq!(warning.source_excerpt, None);
}

#[test]
fn warns_for_list_item_with_code_block_and_continuation_paragraph() {
    let output = markdown_to_jira(
        r#"- item
  ```
  value
  ```
  details"#,
    )
    .unwrap();

    assert_eq!(
        output.markup,
        r#"* item
{code}
value
{code}
details"#
    );
    assert_eq!(output.warnings.len(), 1);
    assert_eq!(output.warnings[0].kind.as_str(), "list-continuation");
    assert!(output.warnings[0].lossy);
}

#[test]
fn converts_blockquote() {
    let (markup, warnings) = convert("> Quoted text\n>\n> Second paragraph");

    assert_eq!(markup, "bq. Quoted text\nbq. \nbq. Second paragraph");
    assert!(warnings.is_empty());
}

#[test]
fn converts_table() {
    let (markup, warnings) =
        convert("| Name | Value |\n| --- | --- |\n| Alpha | *one* |\n| Beta | **two** |");

    assert_eq!(markup, "||Name||Value||\n|Alpha|_one_|\n|Beta|*two*|");
    assert!(warnings.is_empty());
}

#[test]
fn escapes_pipes_inside_table_cells() {
    let (markup, warnings) = convert(
        r#"| Name \| Alias | Value |
| --- | --- |
| a \| b | x |"#,
    );

    assert_eq!(markup, "||Name \\| Alias||Value||\n|a \\| b|x|");
    assert!(warnings.is_empty());
}

#[test]
fn escapes_link_label_and_url_delimiters() {
    let (markup, warnings) = convert(r#"[a \| b\]](<https://x.test/a|b]c>)"#);

    assert_eq!(markup, "[a \\| b\\]|https://x.test/a\\|b\\]c]");
    assert!(warnings.is_empty());
}

#[test]
fn warns_when_converting_image() {
    let (markup, warnings) = convert("![Diagram](https://example.com/diagram.png)");

    assert_eq!(markup, "!https://example.com/diagram.png!");
    assert_eq!(
        warnings,
        vec!["Image alt text dropped; Jira image markup does not preserve Markdown alt text."]
    );
}

#[test]
fn warns_when_preserving_raw_html() {
    let (markup, warnings) = convert("<span data-kind=\"note\">raw</span>");

    assert_eq!(markup, "<span data-kind=\"note\">raw</span>");
    assert_eq!(
        warnings,
        vec![
            "Raw HTML preserved as text; HTML rendering semantics are lossy.",
            "Raw HTML preserved as text; HTML rendering semantics are lossy.",
        ]
    );
}

#[test]
fn warns_when_preserving_task_list_checkbox_state_as_text() {
    let (markup, warnings) = convert("- [x] done\n- [ ] todo");

    assert_eq!(markup, "* \\[x\\] done\n* \\[ \\] todo");
    assert_eq!(
        warnings,
        vec![
            "Task list checkbox state preserved as text; task-list semantics are lossy.",
            "Task list checkbox state preserved as text; task-list semantics are lossy.",
        ]
    );
}
