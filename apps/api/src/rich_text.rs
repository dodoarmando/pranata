//! Rich text: Tiptap/ProseMirror JSON validated against an allowlist and
//! rendered to HTML by the server (D-17).
//!
//! The renderer constructs every tag itself, so nothing from the input passes
//! through as markup.

use std::fmt::Write;

use serde_json::{Map, Value, json};
use thiserror::Error;
use uuid::Uuid;

use crate::media::{MediaRef, MediaVariant};

/// Deepest node nesting accepted, counting `doc` as depth 1.
const MAX_DEPTH: usize = 32;
/// Most nodes accepted in one document.
const MAX_NODES: usize = 200_000;
/// Width the `src` fallback aims for.
const TARGET_WIDTH: i32 = 960;
const SIZES: &str = "(min-width: 768px) 720px, 100vw";
const YOUTUBE_PLACEHOLDER: &str = "/assets/youtube-placeholder.svg";

/// Why a document was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RichTextError {
    #[error("rich text must be a JSON object with type `doc`")]
    NotADocument,
    #[error("rich text is nested too deeply or has too many nodes")]
    TooLarge,
}

/// A sanitized `ProseMirror` document. Only [`validate`] builds one.
#[derive(Debug, Clone, PartialEq)]
pub struct RichTextDoc(Value);

impl RichTextDoc {
    /// The sanitized JSON, ready to store.
    #[must_use]
    pub fn as_value(&self) -> &Value {
        &self.0
    }

    #[must_use]
    pub fn into_value(self) -> Value {
        self.0
    }
}

/// Sanitize `doc` against the allowlist.
///
/// Unknown nodes are dropped (their children are kept), unknown marks and
/// attributes are stripped.
///
/// # Errors
/// [`RichTextError::NotADocument`] when the input is not an object of type
/// `doc`; [`RichTextError::TooLarge`] beyond the depth and size caps.
pub fn validate(doc: &Value) -> Result<RichTextDoc, RichTextError> {
    if doc.get("type").and_then(Value::as_str) != Some("doc") {
        return Err(RichTextError::NotADocument);
    }
    check_limits(doc)?;
    let mut children = Vec::new();
    sanitize_children(doc, &mut children);
    Ok(RichTextDoc(json!({ "type": "doc", "content": children })))
}

/// Iterative, so hostile nesting cannot overflow the stack.
fn check_limits(root: &Value) -> Result<(), RichTextError> {
    let mut stack = vec![(root, 1_usize)];
    let mut nodes = 0_usize;
    while let Some((node, depth)) = stack.pop() {
        nodes += 1;
        if nodes > MAX_NODES || depth > MAX_DEPTH {
            return Err(RichTextError::TooLarge);
        }
        if let Some(children) = node.get("content").and_then(Value::as_array) {
            stack.extend(children.iter().map(|c| (c, depth + 1)));
        }
    }
    Ok(())
}

fn sanitize_children(node: &Value, out: &mut Vec<Value>) {
    if let Some(children) = node.get("content").and_then(Value::as_array) {
        for child in children {
            sanitize_node(child, out);
        }
    }
}

/// Push the sanitized form of `node`, or, when the node itself is not
/// allowed, the sanitized forms of its children.
fn sanitize_node(node: &Value, out: &mut Vec<Value>) {
    // A node without a type is as unknown as any other: keep its children.
    let Some(kind) = node.get("type").and_then(Value::as_str) else {
        return sanitize_children(node, out);
    };
    match kind {
        "text" => {
            let Some(text) = node.get("text").and_then(Value::as_str) else {
                return;
            };
            if text.is_empty() {
                return;
            }
            let mut n = json!({ "type": "text", "text": text });
            let marks = sanitize_marks(node);
            if !marks.is_empty() {
                n["marks"] = Value::Array(marks);
            }
            out.push(n);
        }
        "hardBreak" => out.push(json!({ "type": "hardBreak" })),
        "image" => {
            let attrs = node.get("attrs");
            let id = attrs
                .and_then(|a| a.get("assetId"))
                .and_then(Value::as_str)
                .and_then(|s| Uuid::parse_str(s).ok());
            if let Some(id) = id {
                let mut a = Map::new();
                a.insert("assetId".into(), Value::String(id.to_string()));
                if let Some(alt) = attrs.and_then(|a| a.get("alt")).and_then(Value::as_str) {
                    a.insert("alt".into(), Value::String(alt.to_owned()));
                }
                out.push(json!({ "type": "image", "attrs": a }));
            }
        }
        "youtube" => {
            let id = node
                .get("attrs")
                .and_then(|a| a.get("videoId"))
                .and_then(Value::as_str)
                .filter(|id| is_video_id(id));
            if let Some(id) = id {
                out.push(json!({ "type": "youtube", "attrs": { "videoId": id } }));
            }
        }
        "paragraph" | "bulletList" | "orderedList" | "listItem" | "blockquote" | "table"
        | "tableRow" | "tableHeader" | "tableCell" | "heading" => {
            let mut content = Vec::new();
            sanitize_children(node, &mut content);
            let mut n = json!({ "type": kind, "content": content });
            if kind == "heading" {
                n["attrs"] = json!({ "level": heading_level(node) });
            }
            out.push(n);
        }
        // `doc` nested inside a doc and every unknown node: keep the children.
        _ => sanitize_children(node, out),
    }
}

fn heading_level(node: &Value) -> u8 {
    let level = node
        .get("attrs")
        .and_then(|a| a.get("level"))
        .and_then(Value::as_f64)
        .unwrap_or(2.0);
    // Clamped into 2..=4, so the cast is exact. NaN cannot occur in JSON.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let level = level.clamp(2.0, 4.0).round() as u8;
    level
}

fn is_video_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Allowed marks in canonical nesting order (outermost first).
fn sanitize_marks(node: &Value) -> Vec<Value> {
    let Some(marks) = node.get("marks").and_then(Value::as_array) else {
        return Vec::new();
    };
    let has = |name: &str| {
        marks
            .iter()
            .any(|m| m.get("type").and_then(Value::as_str) == Some(name))
    };
    let mut out = Vec::new();
    let href = marks
        .iter()
        .filter(|m| m.get("type").and_then(Value::as_str) == Some("link"))
        .find_map(|m| {
            m.get("attrs")
                .and_then(|a| a.get("href"))
                .and_then(Value::as_str)
                .and_then(clean_href)
        });
    if let Some(href) = href {
        out.push(json!({ "type": "link", "attrs": { "href": href } }));
    }
    if has("bold") {
        out.push(json!({ "type": "bold" }));
    }
    if has("italic") {
        out.push(json!({ "type": "italic" }));
    }
    out
}

/// The trimmed href when its scheme is allowed and it holds no control chars.
fn clean_href(href: &str) -> Option<String> {
    let href = href.trim();
    if href.chars().any(char::is_control) {
        return None;
    }
    let lower = href.to_ascii_lowercase();
    ["http://", "https://", "mailto:", "tel:"]
        .iter()
        .any(|p| lower.starts_with(p))
        .then(|| href.to_owned())
}

/// Render a sanitized document to HTML. Deterministic; images whose asset
/// `media` cannot resolve are dropped.
#[must_use]
pub fn to_html(doc: &RichTextDoc, media: &dyn Fn(Uuid) -> Option<MediaRef>) -> String {
    let mut out = String::new();
    render_children(&doc.0, media, 1, &mut out);
    out
}

fn render_children(
    node: &Value,
    media: &dyn Fn(Uuid) -> Option<MediaRef>,
    depth: usize,
    out: &mut String,
) {
    // The doc is already bounded; this keeps the renderer safe on its own.
    if depth > MAX_DEPTH {
        return;
    }
    if let Some(children) = node.get("content").and_then(Value::as_array) {
        for child in children {
            render_node(child, media, depth + 1, out);
        }
    }
}

fn render_node(
    node: &Value,
    media: &dyn Fn(Uuid) -> Option<MediaRef>,
    depth: usize,
    out: &mut String,
) {
    let Some(kind) = node.get("type").and_then(Value::as_str) else {
        return;
    };
    let tag = match kind {
        "text" => return render_text(node, out),
        "hardBreak" => return out.push_str("<br>"),
        "image" => return render_image(node, media, out),
        "youtube" => return render_youtube(node, out),
        "paragraph" => "p",
        "bulletList" => "ul",
        "orderedList" => "ol",
        "listItem" => "li",
        "blockquote" => "blockquote",
        "table" => "table",
        "tableRow" => "tr",
        "tableHeader" => "th",
        "tableCell" => "td",
        "heading" => match heading_level(node) {
            2 => "h2",
            3 => "h3",
            _ => "h4",
        },
        _ => return,
    };
    let _ = write!(out, "<{tag}>");
    if tag == "table" {
        out.push_str("<tbody>");
    }
    render_children(node, media, depth, out);
    if tag == "table" {
        out.push_str("</tbody>");
    }
    let _ = write!(out, "</{tag}>");
}

fn render_text(node: &Value, out: &mut String) {
    let text = node.get("text").and_then(Value::as_str).unwrap_or_default();
    let marks = node.get("marks").and_then(Value::as_array);
    let mut closers: Vec<&str> = Vec::new();
    for mark in marks.into_iter().flatten() {
        match mark.get("type").and_then(Value::as_str) {
            Some("link") => {
                let Some(href) = mark
                    .get("attrs")
                    .and_then(|a| a.get("href"))
                    .and_then(Value::as_str)
                    .and_then(clean_href)
                else {
                    continue;
                };
                let web = {
                    let l = href.to_ascii_lowercase();
                    l.starts_with("http://") || l.starts_with("https://")
                };
                out.push_str("<a href=\"");
                escape_into(&href, out);
                out.push_str("\" rel=\"noopener nofollow\"");
                if web {
                    out.push_str(" target=\"_blank\"");
                }
                out.push('>');
                closers.push("</a>");
            }
            Some("bold") => {
                out.push_str("<strong>");
                closers.push("</strong>");
            }
            Some("italic") => {
                out.push_str("<em>");
                closers.push("</em>");
            }
            _ => {}
        }
    }
    escape_into(text, out);
    for closer in closers.iter().rev() {
        out.push_str(closer);
    }
}

fn render_image(node: &Value, media: &dyn Fn(Uuid) -> Option<MediaRef>, out: &mut String) {
    let attrs = node.get("attrs");
    let Some(id) = attrs
        .and_then(|a| a.get("assetId"))
        .and_then(Value::as_str)
        .and_then(|s| Uuid::parse_str(s).ok())
    else {
        return;
    };
    let Some(asset) = media(id) else {
        return;
    };
    let mut variants: Vec<&MediaVariant> = asset
        .variants
        .iter()
        .filter(|v| v.width > 0 && is_media_url(&v.url))
        .collect();
    variants.sort_by(|a, b| a.width.cmp(&b.width).then_with(|| a.url.cmp(&b.url)));
    // Closest to the target; on a tie the larger one wins.
    let Some(src) = variants
        .iter()
        .min_by_key(|v| (v.width.abs_diff(TARGET_WIDTH), std::cmp::Reverse(v.width)))
    else {
        return;
    };
    let alt = attrs
        .and_then(|a| a.get("alt"))
        .and_then(Value::as_str)
        .or(asset.alt.as_deref())
        .unwrap_or_default();

    out.push_str("<img src=\"");
    escape_into(&src.url, out);
    out.push_str("\" srcset=\"");
    for (i, v) in variants.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        escape_into(&v.url, out);
        let _ = write!(out, " {}w", v.width);
    }
    let _ = write!(out, "\" sizes=\"{SIZES}\" alt=\"");
    escape_into(alt, out);
    out.push_str("\" loading=\"lazy\" decoding=\"async\">");
}

/// A click-to-load facade: nothing is requested from `YouTube` until the web
/// script swaps it for a `youtube-nocookie.com` iframe.
fn render_youtube(node: &Value, out: &mut String) {
    let Some(id) = node
        .get("attrs")
        .and_then(|a| a.get("videoId"))
        .and_then(Value::as_str)
        .filter(|id| is_video_id(id))
    else {
        return;
    };
    let _ = write!(
        out,
        "<a class=\"yt-facade\" href=\"https://www.youtube.com/watch?v={id}\" data-video-id=\"{id}\">\
         <img src=\"{YOUTUBE_PLACEHOLDER}\" alt=\"Play video\" loading=\"lazy\" decoding=\"async\"></a>"
    );
}

/// Only site-relative `/media/` paths of safe characters may reach `src` and
/// `srcset`: no foreign hosts (D-20), and no space or comma to break `srcset`.
fn is_media_url(url: &str) -> bool {
    url.starts_with("/media/")
        && url
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'.' | b'_' | b'-'))
}

fn escape_into(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
}
