#![allow(clippy::needless_pass_by_value)] // `json!` borrows its operands, so builders take owned values for brevity.

use pranata::media::{MediaRef, MediaVariant};
use pranata::rich_text::{RichTextError, to_html, validate};
use serde_json::{Value, json};
use uuid::Uuid;

fn render(doc: &Value) -> String {
    render_with(doc, &|_| None)
}

fn render_with(doc: &Value, media: &dyn Fn(Uuid) -> Option<MediaRef>) -> String {
    to_html(&validate(doc).expect("valid document"), media)
}

fn doc(content: Value) -> Value {
    json!({ "type": "doc", "content": content })
}

fn text(t: &str) -> Value {
    json!({ "type": "text", "text": t })
}

fn para(children: Value) -> Value {
    json!({ "type": "paragraph", "content": children })
}

#[test]
fn drops_unknown_nodes_keeps_text() {
    let d = doc(json!([{
        "type": "callout",
        "content": [para(json!([text("kept")]))]
    }]));
    assert_eq!(render(&d), "<p>kept</p>");
}

#[test]
fn javascript_link_mark_removed() {
    for href in [
        "javascript:alert(1)",
        "  JaVaScRiPt:alert(1)",
        "\u{1}javascript:x",
        "//evil.example",
        "/relative",
    ] {
        let d = doc(json!([para(json!([{
            "type": "text", "text": "click",
            "marks": [{ "type": "link", "attrs": { "href": href } }]
        }]))]));
        assert_eq!(render(&d), "<p>click</p>", "href {href:?}");
    }
}

#[test]
fn allowed_links_render_with_rel() {
    let link = |href: &str| {
        doc(json!([para(json!([{
            "type": "text", "text": "x",
            "marks": [{ "type": "link", "attrs": { "href": href } }]
        }]))]))
    };
    assert_eq!(
        render(&link(" HTTPS://example.com/a?b=1&c=2 ")),
        "<p><a href=\"HTTPS://example.com/a?b=1&amp;c=2\" rel=\"noopener nofollow\" target=\"_blank\">x</a></p>"
    );
    assert_eq!(
        render(&link("mailto:a@example.com")),
        "<p><a href=\"mailto:a@example.com\" rel=\"noopener nofollow\">x</a></p>"
    );
    assert_eq!(
        render(&link("tel:+62123")),
        "<p><a href=\"tel:+62123\" rel=\"noopener nofollow\">x</a></p>"
    );
}

#[test]
fn escapes_text() {
    let d = doc(json!([para(json!([text(
        "<script>alert('x')</script> & \"q\""
    )]))]));
    assert_eq!(
        render(&d),
        "<p>&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt; &amp; &quot;q&quot;</p>"
    );
}

#[test]
fn heading_level_1_becomes_2() {
    let h = |level: u32| {
        doc(json!([{ "type": "heading", "attrs": { "level": level }, "content": [text("T")] }]))
    };
    assert_eq!(render(&h(1)), "<h2>T</h2>");
    assert_eq!(render(&h(3)), "<h3>T</h3>");
    assert_eq!(render(&h(6)), "<h4>T</h4>");
}

#[test]
fn youtube_renders_facade_without_iframe() {
    let d = doc(json!([{ "type": "youtube", "attrs": { "videoId": "dQw4w9WgXcQ" } }]));
    let html = render(&d);
    assert!(!html.contains("<iframe"));
    assert!(html.contains(
        "<a class=\"yt-facade\" href=\"https://www.youtube.com/watch?v=dQw4w9WgXcQ\" data-video-id=\"dQw4w9WgXcQ\">"
    ));
    assert!(html.contains("/assets/youtube-placeholder.svg"));
    assert!(!html.contains("ytimg.com"));
}

#[test]
fn invalid_video_id_dropped() {
    for id in ["short", "dQw4w9WgXcQ\"><script>", "dQw4w9WgXc!"] {
        let d = doc(json!([{ "type": "youtube", "attrs": { "videoId": id } }]));
        assert_eq!(render(&d), "", "id {id:?}");
    }
}

#[test]
fn image_uses_variants_srcset() {
    let id = Uuid::new_v4();
    let d =
        doc(json!([{ "type": "image", "attrs": { "assetId": id.to_string(), "alt": "A \"b\"" } }]));
    let media = move |got: Uuid| {
        (got == id).then(|| MediaRef {
            id,
            alt: Some("fallback".into()),
            variants: vec![
                MediaVariant {
                    width: 1600,
                    url: "/media/ab/abc-1600.webp".into(),
                },
                MediaVariant {
                    width: 480,
                    url: "/media/ab/abc-480.webp".into(),
                },
                MediaVariant {
                    width: 1024,
                    url: "/media/ab/abc-1024.webp".into(),
                },
            ],
        })
    };
    let html = render_with(&d, &media);
    assert_eq!(
        html,
        "<img src=\"/media/ab/abc-1024.webp\" srcset=\"/media/ab/abc-480.webp 480w, /media/ab/abc-1024.webp 1024w, /media/ab/abc-1600.webp 1600w\" sizes=\"(min-width: 768px) 720px, 100vw\" alt=\"A &quot;b&quot;\" loading=\"lazy\" decoding=\"async\">"
    );
    // Unresolvable assets are dropped silently.
    assert_eq!(render(&d), "");
    // Alt falls back to the media record.
    let d2 = doc(json!([{ "type": "image", "attrs": { "assetId": id.to_string() } }]));
    assert!(render_with(&d2, &media).contains("alt=\"fallback\""));
}

#[test]
fn pasted_word_styles_ignored() {
    let d = doc(json!([para(json!([{
        "type": "text", "text": "plain",
        "marks": [
            { "type": "textStyle", "attrs": { "color": "#ff0000", "fontFamily": "Calibri" } },
            { "type": "color", "attrs": { "color": "red" } }
        ]
    }]))]));
    assert_eq!(render(&d), "<p>plain</p>");
}

#[test]
fn marks_and_structures_render() {
    let d = doc(json!([
        { "type": "bulletList", "content": [{ "type": "listItem", "content": [para(json!([
            { "type": "text", "text": "b", "marks": [{ "type": "italic" }, { "type": "bold" }] },
            { "type": "hardBreak" }
        ]))] }] },
        { "type": "table", "content": [{ "type": "tableRow", "content": [
            { "type": "tableHeader", "content": [para(json!([text("h")]))] },
            { "type": "tableCell", "content": [para(json!([text("c")]))] }
        ] }] }
    ]));
    assert_eq!(
        render(&d),
        "<ul><li><p><strong><em>b</em></strong><br></p></li></ul><table><tbody><tr><th><p>h</p></th><td><p>c</p></td></tr></tbody></table>"
    );
}

#[test]
fn rejects_non_documents() {
    assert_eq!(
        validate(&json!("x")).unwrap_err(),
        RichTextError::NotADocument
    );
    assert_eq!(
        validate(&json!({ "type": "paragraph" })).unwrap_err(),
        RichTextError::NotADocument
    );
}

#[test]
fn rejects_deep_and_huge_documents() {
    let mut deep = para(json!([text("x")]));
    for _ in 0..40 {
        deep = json!({ "type": "blockquote", "content": [deep] });
    }
    assert_eq!(
        validate(&doc(json!([deep]))).unwrap_err(),
        RichTextError::TooLarge
    );

    let many: Vec<Value> = (0..200_001)
        .map(|_| json!({ "type": "hardBreak" }))
        .collect();
    assert_eq!(
        validate(&doc(Value::Array(many))).unwrap_err(),
        RichTextError::TooLarge
    );
}

fn image_with(urls: &[(i32, &str)]) -> String {
    let id = Uuid::new_v4();
    let d = doc(json!([{ "type": "image", "attrs": { "assetId": id.to_string() } }]));
    let variants: Vec<MediaVariant> = urls
        .iter()
        .map(|&(width, url)| MediaVariant {
            width,
            url: url.to_owned(),
        })
        .collect();
    render_with(&d, &move |_| {
        Some(MediaRef {
            id,
            alt: None,
            variants: variants.clone(),
        })
    })
}

#[test]
fn foreign_or_malformed_variant_urls_are_dropped() {
    for bad in [
        "https://evil.example/x.webp",
        "//evil/x.webp",
        "/media/a b.webp",
        "/media/a,b.webp",
    ] {
        assert_eq!(image_with(&[(960, bad)]), "", "url {bad:?}");
    }
    let html = image_with(&[
        (960, "https://evil.example/x.webp"),
        (480, "/media/ab/abc-480.webp"),
    ]);
    assert!(html.contains("src=\"/media/ab/abc-480.webp\""));
    assert!(!html.contains("evil"));
}

#[test]
fn non_positive_widths_are_skipped() {
    let html = image_with(&[
        (0, "/media/ab/a-0.webp"),
        (-5, "/media/ab/a-5.webp"),
        (480, "/media/ab/a-480.webp"),
    ]);
    assert!(
        html.contains("srcset=\"/media/ab/a-480.webp 480w\""),
        "{html}"
    );
    assert_eq!(image_with(&[(0, "/media/ab/a-0.webp")]), "");
}

#[test]
fn typeless_node_keeps_its_children() {
    let d = doc(json!([{ "content": [para(json!([text("kept")]))] }, { "text": "loose" }]));
    assert_eq!(render(&d), "<p>kept</p>");
}

#[test]
fn heading_level_clamps_floats_and_out_of_range() {
    let h = |level: Value| {
        doc(json!([{ "type": "heading", "attrs": { "level": level }, "content": [text("T")] }]))
    };
    assert_eq!(render(&h(json!(5.0))), "<h4>T</h4>");
    assert_eq!(render(&h(json!(3.0))), "<h3>T</h3>");
    assert_eq!(render(&h(json!(0))), "<h2>T</h2>");
    assert_eq!(render(&h(json!(u64::MAX))), "<h4>T</h4>");
    assert_eq!(render(&h(json!(-7))), "<h2>T</h2>");
}

#[test]
fn validate_returns_sanitized_json() {
    let d = doc(json!([{
        "type": "paragraph",
        "attrs": { "textAlign": "center" },
        "content": [{
            "type": "text", "text": "hi",
            "marks": [
                { "type": "italic" },
                { "type": "color", "attrs": { "color": "red" } },
                { "type": "link", "attrs": { "href": "https://example.com", "target": "_self", "class": "x" } },
                { "type": "bold", "attrs": { "x": 1 } }
            ]
        }]
    }]));
    assert_eq!(
        validate(&d).unwrap().as_value(),
        &json!({ "type": "doc", "content": [{
            "type": "paragraph",
            "content": [{
                "type": "text", "text": "hi",
                "marks": [
                    { "type": "link", "attrs": { "href": "https://example.com" } },
                    { "type": "bold" },
                    { "type": "italic" }
                ]
            }]
        }] })
    );
}

#[test]
fn depth_boundary_is_32() {
    let nested = |quotes: usize| {
        let mut node = para(json!([text("x")]));
        for _ in 0..quotes {
            node = json!({ "type": "blockquote", "content": [node] });
        }
        doc(json!([node]))
    };
    // doc (1) + n blockquotes + paragraph + text = n + 3 levels.
    assert!(validate(&nested(29)).is_ok());
    assert_eq!(validate(&nested(30)).unwrap_err(), RichTextError::TooLarge);
}
