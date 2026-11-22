//! Markdown in the `summary` of an `opensource.toml` card, turned into the HTML the
//! card shows.

use pulldown_cmark::{CowStr, Event, LinkType, Parser, Tag, TagEnd, html};
use pulldown_cmark_escape::{escape_href, escape_html};

/// `markdown` as HTML for a card's summary: paragraphs, line breaks, bullet and
/// numbered lists, links, emphasis and inline code.
///
/// Within a paragraph a single newline is a line break, not a space: in a TOML
/// string a newline is put there on purpose. Links open in a new tab with
/// `rel="noopener"` and no `noreferrer`, like the card's other links. What has no
/// place in a card comes out as plain text: a heading's words, an image's alt text,
/// raw HTML as the characters it is written with, and an indented code block or an
/// HTML block as a paragraph of its lines. Block quotes and rules are dropped
/// around their content.
pub fn summary_html(markdown: &str) -> String {
    let mut events = Vec::new();
    // The text of the code or HTML block being read, if any.
    let mut verbatim: Option<String> = None;
    let markdown = without_shared_indent(markdown);
    for event in Parser::new(markdown.trim()) {
        match event {
            Event::Start(Tag::CodeBlock(_) | Tag::HtmlBlock) => verbatim = Some(String::new()),
            Event::End(TagEnd::CodeBlock | TagEnd::HtmlBlock) => {
                if let Some(text) = verbatim.take() {
                    push_lines(&mut events, &text);
                }
            }
            Event::Text(text) | Event::Html(text) if verbatim.is_some() => {
                if let Some(buffer) = verbatim.as_mut() {
                    buffer.push_str(&text);
                }
            }
            Event::SoftBreak => events.push(Event::HardBreak),
            Event::Html(raw) | Event::InlineHtml(raw) => events.push(Event::Text(raw)),
            Event::Start(Tag::Heading { .. }) => events.push(Event::Start(Tag::Paragraph)),
            Event::End(TagEnd::Heading(_)) => events.push(Event::End(TagEnd::Paragraph)),
            Event::Start(Tag::Image { .. } | Tag::BlockQuote(_))
            | Event::End(TagEnd::Image | TagEnd::BlockQuote(_))
            | Event::Rule => {}
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                ..
            }) => events.push(Event::Html(link_start(link_type, &dest_url, &title).into())),
            Event::End(TagEnd::Link) => events.push(Event::Html("</a>".into())),
            event => events.push(event),
        }
    }
    let mut out = String::new();
    html::push_html(&mut out, events.into_iter());
    out
}

/// `markdown` without the indent all its non-blank lines share, so a summary written
/// as a `"""` string indented to line up with the TOML around it reads as if it
/// weren't. Only spaces and tabs count, each as one.
fn without_shared_indent(markdown: &str) -> String {
    let indent = markdown
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start_matches([' ', '\t']).len())
        .min()
        .unwrap_or(0);
    markdown
        .lines()
        .map(|line| line.get(indent..).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

/// A paragraph holding `text`'s lines, one line break between each.
fn push_lines(events: &mut Vec<Event<'_>>, text: &str) {
    events.push(Event::Start(Tag::Paragraph));
    for (i, line) in text.trim_end_matches('\n').split('\n').enumerate() {
        if i > 0 {
            events.push(Event::HardBreak);
        }
        events.push(Event::Text(CowStr::from(line.to_owned())));
    }
    events.push(Event::End(TagEnd::Paragraph));
}

/// The `<a>` start tag for a link, escaped as pulldown-cmark's own writer escapes it.
fn link_start(link_type: LinkType, dest_url: &str, title: &str) -> String {
    let mut tag = String::from("<a href=\"");
    if link_type == LinkType::Email {
        tag.push_str("mailto:");
    }
    escape_href(&mut tag, dest_url).expect("pushing to a String can't fail");
    if !title.is_empty() {
        tag.push_str("\" title=\"");
        escape_html(&mut tag, title).expect("pushing to a String can't fail");
    }
    tag.push_str("\" target=\"_blank\" rel=\"noopener\">");
    tag
}

#[cfg(test)]
mod tests {
    use super::summary_html;

    #[test]
    fn plain_text_is_one_paragraph() {
        assert_eq!(
            summary_html("A fixture project."),
            "<p>A fixture project.</p>\n"
        );
    }

    #[test]
    fn text_is_escaped() {
        let html = summary_html("Fixes & <improvements>");
        assert!(html.contains("Fixes &amp; &lt;improvements&gt;"), "{html}");
    }

    #[test]
    fn blank_is_empty() {
        assert_eq!(summary_html(""), "");
        assert_eq!(summary_html("  \n "), "");
    }

    #[test]
    fn a_link_opens_in_a_new_tab() {
        assert_eq!(
            summary_html("See [it](https://example.com/a?b=1&c=2)."),
            "<p>See <a href=\"https://example.com/a?b=1&amp;c=2\" target=\"_blank\" \
             rel=\"noopener\">it</a>.</p>\n"
        );
    }

    #[test]
    fn a_link_title_is_escaped_into_its_attribute() {
        assert_eq!(
            summary_html(r#"[t](https://e.example "a \"title\" & <x>")"#),
            "<p><a href=\"https://e.example\" title=\"a &quot;title&quot; &amp; &lt;x&gt;\" \
             target=\"_blank\" rel=\"noopener\">t</a></p>\n"
        );
    }

    #[test]
    fn an_email_autolink_gets_mailto() {
        assert_eq!(
            summary_html("<me@e.example>"),
            "<p><a href=\"mailto:me@e.example\" target=\"_blank\" rel=\"noopener\">\
             me@e.example</a></p>\n"
        );
    }

    #[test]
    fn link_addresses_are_escaped() {
        let html = summary_html("[x](https://e.example/\"onmouseover=alert(1))");
        assert!(html.contains("<a href=\"https://e.example/"), "{html}");
        assert!(!html.contains("\"onmouseover"), "{html}");
    }

    #[test]
    fn a_single_newline_is_a_line_break() {
        assert_eq!(summary_html("one\ntwo"), "<p>one<br />\ntwo</p>\n");
    }

    #[test]
    fn a_blank_line_starts_a_paragraph() {
        assert_eq!(summary_html("one\n\ntwo"), "<p>one</p>\n<p>two</p>\n");
    }

    #[test]
    fn dashes_make_a_bullet_list() {
        assert_eq!(
            summary_html("- a\n- b"),
            "<ul>\n<li>a</li>\n<li>b</li>\n</ul>\n"
        );
    }

    #[test]
    fn numbers_make_a_numbered_list() {
        let html = summary_html("1. a\n2. b");
        assert!(html.contains("<ol>"), "{html}");
        assert!(html.contains("<li>a</li>"), "{html}");
    }

    #[test]
    fn a_list_can_follow_a_line_directly() {
        assert_eq!(
            summary_html("Points:\n- a"),
            "<p>Points:</p>\n<ul>\n<li>a</li>\n</ul>\n"
        );
    }

    #[test]
    fn emphasis_and_code_pass_through() {
        assert_eq!(
            summary_html("*a* **b** `c`"),
            "<p><em>a</em> <strong>b</strong> <code>c</code></p>\n"
        );
    }

    #[test]
    fn raw_html_shows_as_text() {
        let html = summary_html("<b>hi</b> <script>x</script>");
        assert!(html.contains("&lt;b&gt;hi&lt;/b&gt;"), "{html}");
        assert!(!html.contains("<b>"), "{html}");
        assert!(!html.contains("<script"), "{html}");

        let block = summary_html("<div>\nx\n</div>");
        assert!(!block.contains("<div"), "{block}");
        assert!(block.contains("&lt;div&gt;"), "{block}");
    }

    #[test]
    fn headings_and_images_show_as_plain_text() {
        assert_eq!(
            summary_html("# Title\n\n![alt](/x.png)"),
            "<p>Title</p>\n<p>alt</p>\n"
        );
    }

    /// A `"""` string indented to line up with the TOML around it: the shared indent
    /// goes, so the Markdown after the first paragraph still works.
    #[test]
    fn an_indented_summary_keeps_its_markdown() {
        let html = summary_html(
            "\n    Intro.\n\n    See [text](https://b.example).\n\n    - a\n    - b\n    ",
        );
        assert!(html.contains("<a href=\"https://b.example\""), "{html}");
        assert!(html.contains("<li>a</li>"), "{html}");
        assert!(!html.contains("- a"), "{html}");
    }

    /// Lines indented four spaces past the rest are a code block to Markdown; here
    /// they are a paragraph of plain lines. Fenced code too.
    #[test]
    fn indented_lines_stay_text() {
        let html = summary_html("First.\n\n    indented\n    lines");
        assert!(!html.contains("<pre"), "{html}");
        assert!(!html.contains("<code"), "{html}");
        assert!(html.contains("<p>indented<br />\nlines</p>"), "{html}");

        let fenced = summary_html("```\ncode\nmore\n```");
        assert!(!fenced.contains("<pre"), "{fenced}");
        assert!(fenced.contains("<p>code<br />\nmore</p>"), "{fenced}");
    }
}
