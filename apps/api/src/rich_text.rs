//! Rich text: Tiptap's JSON, parsed into types that are the allowlist. A node or mark not
//! named here is refused; an attribute not named here is dropped, since a document is stored
//! as these types write it back.

use serde::{Deserialize, Serialize};

/// A whole document, `{ "type": "doc", "content": [...] }`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Doc {
    Doc {
        #[serde(default)]
        content: Vec<Block>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Block {
    Paragraph {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        content: Vec<Text>,
    },
    Heading {
        attrs: HeadingAttrs,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        content: Vec<Text>,
    },
    BulletList {
        content: Vec<ListItem>,
    },
    OrderedList {
        #[serde(default)]
        attrs: OrderedListAttrs,
        content: Vec<ListItem>,
    },
    Blockquote {
        content: Vec<Block>,
    },
    Table {
        content: Vec<TableRow>,
    },
    /// A `YouTube` video, kept as its id and played from `youtube-nocookie.com`.
    Youtube {
        attrs: YoutubeAttrs,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ListItem {
    ListItem { content: Vec<Block> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TableRow {
    TableRow { content: Vec<TableCell> },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TableCell {
    TableHeader { content: Vec<Block> },
    TableCell { content: Vec<Block> },
}

/// Text is the only inline node: no images, no hard breaks.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Text {
    Text {
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        marks: Vec<Mark>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Mark {
    Bold,
    Italic,
    Link { attrs: LinkAttrs },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct HeadingAttrs {
    level: Level,
}

/// `h2` or `h3`; the page title is the only `h1`.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(try_from = "u8")]
pub struct Level(u8);

impl TryFrom<u8> for Level {
    type Error = &'static str;
    fn try_from(level: u8) -> Result<Self, Self::Error> {
        matches!(level, 2 | 3)
            .then_some(Self(level))
            .ok_or("only h2 and h3")
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct OrderedListAttrs {
    start: u32,
}

impl Default for OrderedListAttrs {
    fn default() -> Self {
        Self { start: 1 }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LinkAttrs {
    href: Href,
}

/// A link target: `http(s):`, `mailto:` or `tel:`, never `javascript:` or `data:`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(try_from = "String")]
pub struct Href(String);

impl TryFrom<String> for Href {
    type Error = &'static str;
    fn try_from(href: String) -> Result<Self, Self::Error> {
        let href = href.trim();
        let lower = href.to_ascii_lowercase();
        ["http://", "https://", "mailto:", "tel:"]
            .iter()
            .any(|scheme| lower.len() > scheme.len() && lower.starts_with(scheme))
            .then(|| Self(href.to_owned()))
            .ok_or("only http(s), mailto and tel links")
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeAttrs {
    video_id: VideoId,
}

/// A `YouTube` video id: 11 characters of `A–Z a–z 0–9 _ -`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(try_from = "String")]
pub struct VideoId(String);

impl TryFrom<String> for VideoId {
    type Error = &'static str;
    fn try_from(id: String) -> Result<Self, Self::Error> {
        (id.len() == 11
            && id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'))
        .then_some(Self(id))
        .ok_or("not a YouTube video id")
    }
}

impl Doc {
    /// An empty document, as a new Draft starts.
    pub fn empty() -> Self {
        Self::Doc {
            content: Vec::new(),
        }
    }

    /// Whether it holds nothing but paragraphs of whitespace at most, as the editor's blank
    /// document does.
    pub fn is_blank(&self) -> bool {
        let Self::Doc { content } = self;
        content.iter().all(|block| match block {
            Block::Paragraph { content } => content
                .iter()
                .all(|Text::Text { text, .. }| text.trim().is_empty()),
            _ => false,
        })
    }

    /// The text alone, one line per block, for reading time and search.
    pub fn plain_text(&self) -> String {
        let Self::Doc { content } = self;
        let mut lines = Vec::new();
        blocks_text(content, &mut lines);
        lines.join("\n")
    }

    /// Minutes to read at 200 words a minute, at least 1.
    pub fn reading_minutes(&self) -> i32 {
        let words = self.plain_text().split_whitespace().count();
        i32::try_from(words.div_ceil(200).max(1)).unwrap_or(i32::MAX)
    }
}

fn blocks_text(blocks: &[Block], lines: &mut Vec<String>) {
    let inline = |content: &[Text]| {
        content
            .iter()
            .map(|Text::Text { text, .. }| text.as_str())
            .collect::<String>()
    };
    for block in blocks {
        match block {
            Block::Paragraph { content } | Block::Heading { content, .. } => {
                lines.push(inline(content));
            }
            Block::BulletList { content } | Block::OrderedList { content, .. } => {
                for ListItem::ListItem { content } in content {
                    blocks_text(content, lines);
                }
            }
            Block::Blockquote { content } => blocks_text(content, lines),
            Block::Table { content } => {
                for TableRow::TableRow { content } in content {
                    for TableCell::TableHeader { content } | TableCell::TableCell { content } in
                        content
                    {
                        blocks_text(content, lines);
                    }
                }
            }
            Block::Youtube { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn parse(content: &Value) -> Result<Doc, serde_json::Error> {
        serde_json::from_value(json!({ "type": "doc", "content": content }))
    }

    #[test]
    fn only_empty_or_whitespace_paragraphs_are_blank() {
        let blank = parse(&json!([
            { "type": "paragraph" },
            { "type": "paragraph", "content": [{ "type": "text", "text": "  " }] },
        ]))
        .unwrap();
        assert!(blank.is_blank());
        let video =
            parse(&json!([{ "type": "youtube", "attrs": { "videoId": "dQw4w9WgXcQ" } }])).unwrap();
        assert!(!video.is_blank());
        let text =
            parse(&json!([{ "type": "paragraph", "content": [{ "type": "text", "text": "x" }] }]))
                .unwrap();
        assert!(!text.is_blank());
    }

    fn paragraph_with_mark(mark: &Value) -> Value {
        json!([{ "type": "paragraph", "content": [
            { "type": "text", "text": "x", "marks": [mark] },
        ] }])
    }

    #[test]
    fn every_allowed_node_and_mark_is_kept_as_tiptap_wrote_it() {
        let content = json!([
            { "type": "heading", "attrs": { "level": 2 }, "content": [{ "type": "text", "text": "Judul" }] },
            { "type": "heading", "attrs": { "level": 3 }, "content": [{ "type": "text", "text": "Sub" }] },
            { "type": "paragraph", "content": [
                { "type": "text", "text": "tebal", "marks": [{ "type": "bold" }, { "type": "italic" }] },
                { "type": "text", "text": "tautan", "marks": [{ "type": "link", "attrs": { "href": "https://desa.example/a" } }] },
            ] },
            { "type": "paragraph" },
            { "type": "bulletList", "content": [
                { "type": "listItem", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "satu" }] }] },
            ] },
            { "type": "orderedList", "attrs": { "start": 3 }, "content": [
                { "type": "listItem", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "tiga" }] }] },
            ] },
            { "type": "blockquote", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "kutipan" }] }] },
            { "type": "table", "content": [
                { "type": "tableRow", "content": [
                    { "type": "tableHeader", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Kolom" }] }] },
                ] },
                { "type": "tableRow", "content": [
                    { "type": "tableCell", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Isi" }] }] },
                ] },
            ] },
            { "type": "youtube", "attrs": { "videoId": "dQw4w9WgXcQ" } },
        ]);

        let doc = parse(&content).unwrap();

        assert_eq!(
            serde_json::to_value(&doc).unwrap(),
            json!({ "type": "doc", "content": content })
        );
    }

    #[test]
    fn attributes_outside_the_allowlist_are_dropped() {
        let doc = parse(&json!([
            { "type": "orderedList", "attrs": { "start": 1, "type": null }, "content": [
                { "type": "listItem", "content": [{ "type": "paragraph", "content": [
                    { "type": "text", "text": "x", "marks": [{ "type": "link", "attrs": {
                        "href": "mailto:kades@desa.example", "target": "_blank",
                        "onclick": "alert(1)", "class": null,
                    } }] },
                ] }] },
            ] },
        ]))
        .unwrap();

        assert_eq!(
            serde_json::to_value(&doc).unwrap()["content"][0],
            json!({ "type": "orderedList", "attrs": { "start": 1 }, "content": [
                { "type": "listItem", "content": [{ "type": "paragraph", "content": [
                    { "type": "text", "text": "x", "marks": [{ "type": "link", "attrs": {
                        "href": "mailto:kades@desa.example",
                    } }] },
                ] }] },
            ] })
        );
    }

    #[test]
    fn nodes_outside_the_allowlist_are_refused() {
        for node in [
            json!({ "type": "image", "attrs": { "src": "https://x.example/a.png" } }),
            json!({ "type": "codeBlock", "content": [{ "type": "text", "text": "x" }] }),
            json!({ "type": "horizontalRule" }),
            json!({ "type": "heading", "attrs": { "level": 1 }, "content": [] }),
            json!({ "type": "heading", "attrs": { "level": 4 }, "content": [] }),
            json!({ "type": "paragraph", "content": [{ "type": "hardBreak" }] }),
            json!({ "type": "paragraph", "content": [{ "type": "image", "attrs": { "src": "x" } }] }),
            json!({ "type": "youtube", "attrs": { "src": "https://youtube.com/watch?v=dQw4w9WgXcQ" } }),
            json!({ "type": "youtube", "attrs": { "videoId": "dQw4w9WgXc\"" } }),
            json!({ "type": "youtube", "attrs": { "videoId": "short" } }),
        ] {
            assert!(parse(&json!([node])).is_err(), "{node}");
        }
        assert!(serde_json::from_value::<Doc>(json!({ "type": "paragraph" })).is_err());
    }

    #[test]
    fn marks_outside_the_allowlist_are_refused() {
        for mark in [
            json!({ "type": "underline" }),
            json!({ "type": "strike" }),
            json!({ "type": "code" }),
            json!({ "type": "textStyle", "attrs": { "color": "red" } }),
        ] {
            assert!(parse(&paragraph_with_mark(&mark)).is_err(), "{mark}");
        }
    }

    #[test]
    fn links_go_only_to_the_web_mail_and_phone() {
        for href in [
            "http://desa.example",
            "HTTPS://desa.example/berita?a=1",
            "mailto:kades@desa.example",
            "tel:+62211234567",
        ] {
            let mark = json!({ "type": "link", "attrs": { "href": href } });
            assert!(parse(&paragraph_with_mark(&mark)).is_ok(), "{href}");
        }
        for href in [
            "javascript:alert(1)",
            " JavaScript:alert(1)",
            "data:text/html,<script>alert(1)</script>",
            "vbscript:msgbox",
            "/berita/relatif",
            "//evil.example",
            "https://",
            "",
        ] {
            let mark = json!({ "type": "link", "attrs": { "href": href } });
            assert!(parse(&paragraph_with_mark(&mark)).is_err(), "{href}");
        }
    }

    #[test]
    fn plain_text_and_reading_time_come_from_every_block() {
        let words = vec!["kata"; 401].join(" ");
        let doc = parse(&json!([
            { "type": "heading", "attrs": { "level": 2 }, "content": [{ "type": "text", "text": "Judul" }] },
            { "type": "paragraph", "content": [
                { "type": "text", "text": "Gotong " }, { "type": "text", "text": "royong", "marks": [{ "type": "bold" }] },
            ] },
            { "type": "table", "content": [{ "type": "tableRow", "content": [
                { "type": "tableCell", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": "sel" }] }] },
            ] }] },
            { "type": "youtube", "attrs": { "videoId": "dQw4w9WgXcQ" } },
        ]))
        .unwrap();
        let long = parse(&json!([
            { "type": "blockquote", "content": [{ "type": "paragraph", "content": [{ "type": "text", "text": words }] }] },
        ]))
        .unwrap();

        assert_eq!(doc.plain_text(), "Judul\nGotong royong\nsel");
        assert_eq!(doc.reading_minutes(), 1);
        assert_eq!(Doc::empty().reading_minutes(), 1);
        assert_eq!(long.reading_minutes(), 3);
    }
}
