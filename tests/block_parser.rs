use jira_parser::{parse_jira, Block, Document, Inline, ListItem};

fn raw_text(text: &str) -> Vec<Inline> {
    vec![Inline::Text(text.to_string())]
}

#[test]
fn parses_full_sample_input() {
    let input = "\
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
{code}";

    let document = parse_jira(input).unwrap();

    assert_eq!(
        document,
        Document {
            blocks: vec![
                Block::Heading {
                    level: 1,
                    content: raw_text("Level 1 Title"),
                },
                Block::Paragraph {
                    content: raw_text("This is an ordered list:"),
                },
                Block::OrderedList {
                    items: vec![
                        ListItem {
                            blocks: vec![Block::Paragraph {
                                content: raw_text("first"),
                            }],
                        },
                        ListItem {
                            blocks: vec![Block::Paragraph {
                                content: raw_text("second"),
                            }],
                        },
                    ],
                },
                Block::Paragraph {
                    content: raw_text("This is an unordered list:"),
                },
                Block::UnorderedList {
                    items: vec![
                        ListItem {
                            blocks: vec![Block::Paragraph {
                                content: raw_text("apple"),
                            }],
                        },
                        ListItem {
                            blocks: vec![Block::Paragraph {
                                content: raw_text("banana"),
                            }],
                        },
                    ],
                },
                Block::Paragraph {
                    content: raw_text("This is a code block"),
                },
                Block::CodeBlock {
                    text: "const obj = {};".to_string(),
                },
            ],
        }
    );
}
