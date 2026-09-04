#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Heading { level: u8, content: Vec<Inline> },
    Paragraph { content: Vec<Inline> },
    OrderedList { items: Vec<ListItem> },
    UnorderedList { items: Vec<ListItem> },
    CodeBlock { text: String },
    NoFormatBlock { text: String },
    Quote { blocks: Vec<Block> },
    Table { rows: Vec<TableRow> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    Strong(Vec<Inline>),
    Emphasis(Vec<Inline>),
    Inserted(Vec<Inline>),
    Strikethrough(Vec<Inline>),
    Color {
        color: String,
        content: Vec<Inline>,
    },
    Link {
        text: Option<Vec<Inline>>,
        url: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCell {
    pub header: bool,
    pub content: Vec<Inline>,
}
