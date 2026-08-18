use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd, html};

fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub type Nodes = std::collections::BTreeMap<String, String>;

enum Piece<'a> {
    Prose(String),
    Kept(Event<'a>),
}

fn walk(markdown: &str) -> Vec<Piece<'_>> {
    let mut out: Vec<Piece> = Vec::new();
    let mut skipping = false;
    let mut verbatim = false;

    for event in Parser::new_ext(markdown, options()) {
        if matches!(event, Event::Start(Tag::MetadataBlock(_))) {
            skipping = true;
            continue;
        }
        if matches!(event, Event::End(TagEnd::MetadataBlock(_))) {
            skipping = false;
            continue;
        }
        if skipping {
            continue;
        }
        if matches!(event, Event::Start(Tag::CodeBlock(_))) {
            verbatim = true;
        }
        if matches!(event, Event::End(TagEnd::CodeBlock)) {
            verbatim = false;
        }
        match event {
            Event::Text(text) if !verbatim => match out.last_mut() {
                Some(Piece::Prose(buf)) => buf.push_str(&text),
                _ => out.push(Piece::Prose(text.into_string())),
            },
            other => out.push(Piece::Kept(other)),
        }
    }
    out
}

fn found_in(text: &str) -> Vec<(std::ops::Range<usize>, String)> {
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(open) = text[from..].find("[[") {
        let start = from + open;
        let after = start + 2;
        let Some(close) = text[after..].find("]]") else {
            break;
        };
        let name = text[after..after + close].trim();
        if name.is_empty() || name.contains('\n') || name.contains('[') {
            from = after;
            continue;
        }
        out.push((start..after + close + 2, name.to_owned()));
        from = after + close + 2;
    }
    out
}

fn linked(text: &str, nodes: &Nodes) -> Vec<Event<'static>> {
    let hits = found_in(text);
    if hits.is_empty() {
        return vec![Event::Text(CowStr::from(text.to_owned()))];
    }
    let mut out = Vec::new();
    let mut at = 0usize;
    for (span, name) in hits {
        if span.start > at {
            out.push(Event::Text(CowStr::from(text[at..span.start].to_owned())));
        }
        let target = match nodes.get(&name) {
            Some(node) => format!(" data-node=\"{}\"", escape(node)),
            None => String::new(),
        };
        out.push(Event::InlineHtml(CowStr::from(format!(
            "<a class=\"wiki\" href=\"#\"{target}>{}</a>",
            escape(&name)
        ))));
        at = span.end;
    }
    if at < text.len() {
        out.push(Event::Text(CowStr::from(text[at..].to_owned())));
    }
    out
}

pub fn to_html(markdown: &str, nodes: &Nodes) -> String {
    let mut events: Vec<Event> = Vec::new();
    for piece in walk(markdown) {
        match piece {
            Piece::Prose(text) => events.extend(linked(&text, nodes)),
            Piece::Kept(event) => events.push(event),
        }
    }
    let mut out = String::new();
    html::push_html(&mut out, events.into_iter());
    out
}

pub fn wikilinks(markdown: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for piece in walk(markdown) {
        let Piece::Prose(text) = piece else { continue };
        for (_, name) in found_in(&text) {
            if !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frontmatter_is_not_part_of_what_a_reader_sees() {
        let html = to_html(
            "---\nabout: src/a.rs#b\nwatch: [sig]\n---\n\n# Title\n\nBody.\n",
            &Nodes::new(),
        );
        assert!(html.contains("<h1>Title</h1>"), "{html}");
        assert!(
            !html.contains("about:"),
            "the frontmatter leaked into the page: {html}"
        );
        assert!(!html.contains("watch:"), "{html}");
    }

    fn known(name: &str, node: &str) -> Nodes {
        Nodes::from([(name.to_owned(), node.to_owned())])
    }

    #[test]
    fn a_wikilink_becomes_something_the_page_can_navigate_to() {
        let nodes = known("ast-naming", "memory:memories/ast-naming.md");
        let html = to_html("see [[ast-naming]] for why", &nodes);
        assert!(
            html.contains("data-node=\"memory:memories/ast-naming.md\""),
            "{html}"
        );
        assert!(html.contains(">ast-naming</a>"), "{html}");
    }

    #[test]
    fn a_name_nothing_answers_to_is_written_out_without_a_destination() {
        let html = to_html("see [[ast-naming]] for why", &Nodes::new());
        assert!(
            !html.contains("data-node"),
            "a link is only a link once somethinganswers to the name. Synthesising a target from \
             the name — `memories/<name>.md` — is right for exactly one store and silently \
             wrong for every other, and the symptom is a page full of edges that lead nowhere \
             or, worse, none at all: {html}"
        );
        assert!(html.contains(">ast-naming</a>"), "{html}");
    }

    #[test]
    fn the_brackets_arrive_split_across_events_and_are_still_read_as_one_name() {
        let split = Parser::new_ext("[[a-b]]", options())
            .filter(|e| matches!(e, Event::Text(_)))
            .count();
        assert!(
            split > 1,
            "the parser stopped splitting brackets; this test guards the reason walk() merges"
        );
        assert_eq!(wikilinks("[[a-b]]"), vec!["a-b"]);
    }

    #[test]
    fn a_wikilink_inside_code_stays_literal_because_it_is_being_quoted_not_used() {
        let html = to_html("```\nwrite [[name]] to link\n```\n", &Nodes::new());
        assert!(!html.contains("data-node"), "{html}");
        assert_eq!(wikilinks("```\n[[name]]\n```\n"), Vec::<String>::new());
    }

    #[test]
    fn every_distinct_target_is_reported_once_however_often_it_is_named() {
        let links = wikilinks("[[a]] then [[b]] then [[a]] again");
        assert_eq!(links, vec!["a", "b"]);
    }

    #[test]
    fn an_unclosed_bracket_pair_is_text_and_does_not_swallow_the_rest() {
        let html = to_html("a [[ unclosed and then some prose", &Nodes::new());
        assert!(!html.contains("data-node"), "{html}");
        assert!(html.contains("unclosed and then some prose"), "{html}");
    }

    #[test]
    fn markup_in_a_name_cannot_escape_the_attribute_it_is_written_into() {
        let html = to_html("[[a\"onerror=x]]", &Nodes::new());
        assert!(!html.contains("\"onerror=x"), "{html}");
        assert!(html.contains("&quot;onerror=x"), "{html}");
    }

    #[test]
    fn merging_does_not_reach_across_inline_code() {
        let html = to_html("`[[a` and `b]]`", &Nodes::new());
        assert!(!html.contains("data-node"), "{html}");
    }
}
