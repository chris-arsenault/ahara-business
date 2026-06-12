use scraper::{ElementRef, Html, node::Node};

pub fn select_body_text(plain: Option<&str>, html: Option<&str>) -> String {
    if let Some(plain) = plain.map(str::trim).filter(|plain| !plain.is_empty()) {
        return normalize_text(plain);
    }

    html.map(html_to_text).unwrap_or_default()
}

pub fn html_to_text(html: &str) -> String {
    let fragment = Html::parse_fragment(html);
    let mut output = String::new();
    render_element(fragment.root_element(), &mut output);
    normalize_lines(&output)
}

fn render_element(element: ElementRef<'_>, output: &mut String) {
    let name = element.value().name();
    if should_skip_element(name) {
        return;
    }

    if name == "li" {
        push_break(output);
        push_text(output, "-");
    } else if is_block_element(name) {
        push_break(output);
    }

    for child in element.children() {
        match child.value() {
            Node::Text(text) => push_text(output, text),
            Node::Element(_) => {
                if let Some(element) = ElementRef::wrap(child) {
                    render_element(element, output);
                }
            }
            _ => {}
        }
    }

    if name == "a"
        && let Some(href) = element
            .value()
            .attr("href")
            .map(str::trim)
            .filter(|href| !href.is_empty())
    {
        push_text(output, href);
    }

    if is_block_element(name) {
        push_break(output);
    }
}

fn should_skip_element(name: &str) -> bool {
    matches!(
        name,
        "script" | "style" | "link" | "img" | "iframe" | "object" | "embed" | "svg"
    )
}

fn is_block_element(name: &str) -> bool {
    matches!(
        name,
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "br"
            | "div"
            | "footer"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "li"
            | "main"
            | "ol"
            | "p"
            | "pre"
            | "section"
            | "table"
            | "tr"
            | "ul"
    )
}

fn push_text(output: &mut String, text: &str) {
    for word in text.split_whitespace() {
        if output.ends_with('-') || (needs_space(output) && !starts_with_closing_punctuation(word))
        {
            output.push(' ');
        }
        output.push_str(word);
    }
}

fn push_break(output: &mut String) {
    let trimmed_len = output.trim_end_matches([' ', '\n']).len();
    output.truncate(trimmed_len);
    if output.is_empty() || output.ends_with("\n\n") {
        return;
    }
    if output.ends_with('\n') {
        output.push('\n');
    } else {
        output.push_str("\n\n");
    }
}

fn needs_space(output: &str) -> bool {
    !output.is_empty() && !output.ends_with([' ', '\n'])
}

fn starts_with_closing_punctuation(word: &str) -> bool {
    word.starts_with(['.', ',', ':', ';', '!', '?', ')', ']'])
}

fn normalize_text(text: &str) -> String {
    text.lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn normalize_lines(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::{html_to_text, select_body_text};

    #[test]
    fn inbound_text_prefers_plain_text_body() {
        let body = select_body_text(
            Some(" Plain body\nwith spacing. "),
            Some("<p>HTML body</p>"),
        );

        assert_eq!(body, "Plain body\nwith spacing.");
    }

    #[test]
    fn inbound_text_converts_html_to_plaintext() {
        let body =
            html_to_text("<p>Hello <strong>there</strong>.</p><ul><li>One</li><li>Two</li></ul>");

        assert!(body.contains("Hello there."));
        assert!(body.contains("- One"));
        assert!(body.contains("- Two"));
        assert!(!body.contains("<strong>"));
    }

    #[test]
    fn inbound_text_removes_scripts_styles_and_remote_references() {
        let body = html_to_text(
            r#"
            <style>body { background: url(https://tracker.example/pixel); }</style>
            <script>alert("xss")</script>
            <p>Visible</p>
            <img src="https://tracker.example/pixel.png" alt="tracking pixel">
            <link rel="stylesheet" href="https://tracker.example/style.css">
            "#,
        );

        assert!(body.contains("Visible"));
        assert!(!body.contains("alert"));
        assert!(!body.contains("background"));
        assert!(!body.contains("tracker.example"));
        assert!(!body.contains("tracking pixel"));
    }

    #[test]
    fn inbound_text_preserves_link_urls_as_inert_text() {
        let body = html_to_text(r#"<p>Open <a href="https://example.test/path">the link</a>.</p>"#);

        assert!(body.contains("Open the link https://example.test/path."));
        assert!(!body.contains("<a"));
    }

    #[test]
    fn inbound_text_keeps_dangerous_links_as_plain_text_only() {
        let body = html_to_text(
            r#"<p>Bad <a href="javascript:alert(1)">click</a> and <a href="data:text/html,hi">data</a>.</p>"#,
        );

        assert!(body.contains("javascript:alert(1)"));
        assert!(body.contains("data:text/html,hi"));
        assert!(!body.contains("href="));
        assert!(!body.contains("<script"));
    }
}
