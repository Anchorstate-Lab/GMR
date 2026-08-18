use crate::graph::{AtlasError, Graph};

const LAYOUT_BASE: &str = include_str!("../assets/layout-base.min.js");
const COSE_BASE: &str = include_str!("../assets/cose-base.min.js");
const CYTOSCAPE: &str = include_str!("../assets/cytoscape.min.js");
const FCOSE: &str = include_str!("../assets/cytoscape-fcose.min.js");
const STYLE: &str = include_str!("../assets/atlas.css");
const SCRIPT: &str = include_str!("../assets/atlas.js");

fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn render(graph: &Graph) -> Result<String, AtlasError> {
    graph.check()?;

    let data = serde_json::to_string(graph)
        .expect("a Graph holds only strings, enums and vectors, so it cannot fail to serialize")
        .replace('<', "\\u003c");

    let mut out = String::with_capacity(
        data.len() + LAYOUT_BASE.len() + COSE_BASE.len() + CYTOSCAPE.len() + FCOSE.len() + 8192,
    );

    out.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    out.push_str("<title>");
    out.push_str(&escape_text(&graph.title));
    out.push_str("</title>\n<style>\n");
    out.push_str(STYLE);
    out.push_str("\n</style>\n</head>\n<body>\n");

    out.push_str("<header class=\"bar\">\n<div class=\"brand\">\n");
    if let Some(logo) = &graph.logo {
        out.push_str("<img class=\"mark\" alt=\"\" src=\"");
        out.push_str(&escape_text(logo));
        out.push_str("\">\n");
    }
    out.push_str("<div class=\"brand-text\"><h1>");
    out.push_str(&escape_text(&graph.title));
    out.push_str("</h1><p>");
    out.push_str(&escape_text(&graph.subtitle));
    out.push_str("</p></div>\n</div>\n<div class=\"stats\" id=\"stats\"></div>\n</header>\n");

    out.push_str("<main class=\"stage\">\n");
    out.push_str(
        "<aside class=\"rail\" id=\"rail\">\n\
         <div class=\"rail-head\">\
         <input id=\"q\" class=\"search\" type=\"search\" placeholder=\"filter by name or path\" \
         autocomplete=\"off\" aria-label=\"filter by name or path\">\
         <div class=\"chips tones\" id=\"tones\"></div>\
         </div>\n\
         <div class=\"list\" id=\"list\" role=\"listbox\" aria-label=\"anchors\"></div>\n\
         </aside>\n\
         <div class=\"grip\" id=\"grip-rail\" role=\"separator\" aria-orientation=\"vertical\" \
         tabindex=\"0\" aria-label=\"resize the anchor list\"></div>\n",
    );
    out.push_str(
        "<section class=\"canvas\">\n<div id=\"cy\"></div>\n\
         <div class=\"legend\" id=\"legend\"></div>\n\
         <div class=\"hint\">scroll to zoom · drag to pan · click a node to open it</div>\n\
         <button class=\"refit\" id=\"refit\" type=\"button\">Fit</button>\n\
         </section>\n",
    );
    out.push_str(
        "<div class=\"grip\" id=\"grip-panel\" role=\"separator\" aria-orientation=\"vertical\" \
         tabindex=\"0\" aria-label=\"resize the memory panel\"></div>\n\
         <aside class=\"panel\" id=\"panel\"></aside>\n</main>\n",
    );

    out.push_str("<script id=\"atlas-data\" type=\"application/json\">");
    out.push_str(&data);
    out.push_str("</script>\n");

    for lib in [LAYOUT_BASE, COSE_BASE, CYTOSCAPE, FCOSE] {
        out.push_str("<script>\n");
        out.push_str(lib);
        out.push_str("\n</script>\n");
    }
    out.push_str("<script>\n");
    out.push_str(SCRIPT);
    out.push_str("\n</script>\n</body>\n</html>\n");

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Edge, EdgeKind, Kind, Node, Tone};

    fn graph() -> Graph {
        Graph {
            title: "Atlas".to_owned(),
            subtitle: "test".to_owned(),
            logo: None,
            nodes: vec![
                Node::new("a:x", "x", Kind::Anchor, Tone::Notice).badge("drifted"),
                Node::new("m:y", "y", Kind::Memory, Tone::Calm),
            ],
            edges: vec![Edge::new("m:y", "a:x", EdgeKind::Binding)],
        }
    }

    #[test]
    fn the_page_carries_every_library_it_needs_and_asks_the_network_for_nothing() {
        let html = render(&graph()).unwrap();
        assert!(html.contains("cytoscape"), "cytoscape is not inlined");
        assert!(html.contains("fcose"), "the layout is not inlined");

        for reaches_out in [
            "<script src",
            "<link ",
            "src=\"http",
            "src=\"/",
            "@import",
            "url(http",
        ] {
            assert!(
                !html.contains(reaches_out),
                "the page would go and fetch `{reaches_out}`, which a file:// open cannot serve"
            );
        }
    }

    #[test]
    fn a_memory_that_mentions_a_closing_script_tag_does_not_end_the_data_early() {
        let mut g = graph();
        g.nodes[1].detail = Some("<p>close it like this: &lt;/script&gt; ok</p>".to_owned());
        let html = render(&g).unwrap();

        let start = html.find("type=\"application/json\">").unwrap();
        let block = &html[start..];
        let end = block.find("</script>").unwrap();
        let json = &block["type=\"application/json\">".len()..end];

        let back: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(
            back["nodes"][1]["detail"],
            serde_json::json!("<p>close it like this: &lt;/script&gt; ok</p>"),
            "the data did not survive the round trip through the page"
        );
    }

    #[test]
    fn a_title_carrying_markup_is_escaped_into_the_head() {
        let mut g = graph();
        g.title = "a <b>bold</b> repo".to_owned();
        let html = render(&g).unwrap();
        assert!(html.contains("<title>a &lt;b&gt;bold&lt;/b&gt; repo</title>"));
    }

    #[test]
    fn a_logo_rides_along_in_the_page_instead_of_being_pointed_at() {
        let mut g = graph();
        g.logo = Some("data:image/png;base64,AAAA".to_owned());
        let html = render(&g).unwrap();
        assert!(
            html.contains("src=\"data:image/png;base64,AAAA\""),
            "{}",
            &html[..400]
        );

        assert!(
            !render(&graph()).unwrap().contains("<img"),
            "a page given no logo should not draw an empty one"
        );
    }

    #[test]
    fn a_graph_that_would_not_load_is_refused_instead_of_written_out_broken() {
        let mut g = graph();
        g.edges.push(Edge::new("m:y", "a:gone", EdgeKind::Binding));
        assert!(render(&g).is_err());
    }
}
