use scraper::{Html, Node};

use crate::retriever::{RetrievalError, RetrievalErrorCode};

pub fn normalize_html(bytes: &[u8], maximum: usize) -> Result<String, RetrievalError> {
    let source = std::str::from_utf8(bytes)
        .map_err(|_| RetrievalError::new(RetrievalErrorCode::CharsetForbidden))?;
    let document = Html::parse_document(source);
    let root = document.tree.root();
    let mut output = String::new();
    for node in root.descendants() {
        let ignored = node.ancestors().any(|ancestor| matches!(ancestor.value(), Node::Element(element) if matches!(element.name(), "head" | "script" | "style" | "noscript" | "svg" | "canvas" | "template")));
        if ignored {
            continue;
        }
        match node.value() {
            Node::Text(text) => {
                output.push_str(text);
                output.push(' ');
            }
            Node::Element(element)
                if matches!(
                    element.name(),
                    "p" | "div"
                        | "section"
                        | "article"
                        | "header"
                        | "footer"
                        | "main"
                        | "li"
                        | "br"
                        | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "h5"
                        | "h6"
                ) =>
            {
                output.push('\n')
            }
            _ => {}
        }
        if output.len() > maximum.saturating_mul(8) {
            return Err(RetrievalError::new(
                RetrievalErrorCode::NormalizedSizeExceeded,
            ));
        }
    }
    normalize_whitespace(&output, maximum)
}

pub fn normalize_plain_text(bytes: &[u8], maximum: usize) -> Result<String, RetrievalError> {
    let source = std::str::from_utf8(bytes)
        .map_err(|_| RetrievalError::new(RetrievalErrorCode::CharsetForbidden))?;
    normalize_whitespace(source, maximum)
}

fn normalize_whitespace(input: &str, maximum: usize) -> Result<String, RetrievalError> {
    let mut lines = Vec::new();
    for line in input.lines() {
        let normalized = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if !normalized.is_empty() && lines.last() != Some(&normalized) {
            lines.push(normalized);
        }
    }
    let output = lines.join("\n");
    if output.len() > maximum {
        return Err(RetrievalError::new(
            RetrievalErrorCode::NormalizedSizeExceeded,
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn html_is_structural_deterministic_plain_text() {
        let html = b"<!-- hidden --><h1>Tools &amp; Docs</h1><style>bad</style><p>Hello <b>world</b>.</p><ul><li>One</li><li>Two</li></ul><script>alert(1)</script>";
        let first = normalize_html(html, 1024).unwrap();
        assert_eq!(first, normalize_html(html, 1024).unwrap());
        assert!(first.contains("Tools & Docs"));
        assert!(first.contains("Hello world ."));
        assert!(!first.contains("alert"));
        assert!(!first.contains('<'));
    }
    #[test]
    fn malformed_html_and_unicode_are_deterministic() {
        let html = "<p>Déploiement<p>Kubernetes".as_bytes();
        assert_eq!(
            normalize_html(html, 1024).unwrap(),
            "Déploiement\nKubernetes"
        );
    }
    #[test]
    fn normalized_utf8_bytes_are_bounded() {
        assert_eq!(normalize_plain_text(" é  é ".as_bytes(), 5).unwrap(), "é é");
        assert!(normalize_plain_text("é é".as_bytes(), 4).is_err());
    }
    #[test]
    fn e1_fixtures_are_deterministic_and_do_not_copy_web_pages() {
        let valid = include_bytes!("../tests/fixtures/e1/valid.html");
        let malformed = include_bytes!("../tests/fixtures/e1/malformed.html");
        assert!(
            normalize_html(valid, 16 * 1024)
                .unwrap()
                .contains("Official Tool")
        );
        assert_eq!(
            normalize_html(malformed, 16 * 1024).unwrap(),
            normalize_html(malformed, 16 * 1024).unwrap()
        );
        assert!(
            normalize_plain_text(include_bytes!("../tests/fixtures/e1/valid.txt"), 16 * 1024)
                .is_ok()
        );
    }
}
