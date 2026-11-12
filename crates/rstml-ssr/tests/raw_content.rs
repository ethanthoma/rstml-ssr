use rstml_ssr::{RenderError, TrustedHtml, html, render, render_with_limit};
use scraper::{Html, Selector};

#[test]
fn foreign_elements_preserve_names_namespaces_and_escaped_content() {
    let label = "<&>\"";
    let page = html! {
        <svg viewBox="0 0 24 24" aria-label={label}>
            <path d="M0 0L24 24" />
            <source />
            <foreignObject>
                <div>{label}<br /></div>
            </foreignObject>
        </svg>
        <math>
            <mi>{label}</mi>
            <mo>"<"</mo>
            <mn>{2_u32}</mn>
            <mtext>
                <mglyph />
                <malignmark />
                <span>"HTML text"</span>
            </mtext>
            <annotation-xml>
                <svg>
                    <foreignObject>
                        <div>"Embedded"<br /></div>
                    </foreignObject>
                </svg>
            </annotation-xml>
            <annotation-xml encoding="text/html">
                <div>"HTML"<br /></div>
            </annotation-xml>
        </math>
    };
    let output = render(&page).unwrap();
    assert!(output.as_str().contains("<source></source>"));
    assert!(!output.as_str().contains("</br>"));
    let document = Html::parse_fragment(output.as_str());
    for (name, namespace) in [
        ("svg", "http://www.w3.org/2000/svg"),
        ("path", "http://www.w3.org/2000/svg"),
        ("math", "http://www.w3.org/1998/Math/MathML"),
        ("mglyph", "http://www.w3.org/1998/Math/MathML"),
        ("malignmark", "http://www.w3.org/1998/Math/MathML"),
        ("div", "http://www.w3.org/1999/xhtml"),
    ] {
        let selector = Selector::parse(name).unwrap();
        let elements = document.select(&selector).collect::<Vec<_>>();
        assert!(!elements.is_empty(), "missing {name}");
        for element in elements {
            assert_eq!(
                &*element.value().name.ns,
                namespace,
                "{name}: {}",
                document.html()
            );
        }
    }
    let svg = document
        .select(&Selector::parse("svg").unwrap())
        .next()
        .unwrap();
    assert_eq!(svg.value().attr("viewBox"), Some("0 0 24 24"));
    assert_eq!(svg.value().attr("aria-label"), Some(label));
    assert_eq!(render_with_limit(&page, 1), Err(RenderError::OutputLimit));
}

#[test]
fn inline_code_and_json_are_not_html_escaped() {
    let json = r#"{"name":"A&B","value":"\u003c/script>"}"#;
    let script = "if (1 < 2) window.example = 'A&B';";
    let style = "p > span { color: red; }";
    let page = html! {
        <script>"if (1 < 2) window.example = 'A&B';"</script>
        <style>"p > span { color: red; }"</style>
        <script type="application/ld+json">{TrustedHtml(json)}</script>
    };
    let output = render(&page).unwrap();
    let document = Html::parse_fragment(output.as_str());
    let scripts = document
        .select(&Selector::parse("script").unwrap())
        .map(|element| element.text().collect::<String>())
        .collect::<Vec<_>>();
    assert_eq!(scripts, [script, json]);
    assert_eq!(
        document
            .select(&Selector::parse("style").unwrap())
            .next()
            .unwrap()
            .text()
            .collect::<String>(),
        style
    );
    assert_eq!(render(&page).unwrap(), output);
    assert_eq!(render_with_limit(&page, 1), Err(RenderError::OutputLimit));
}

#[test]
fn foreign_code_uses_character_references_without_changing_parsed_text() {
    let code = "if (1 < 2) window.example = '</script>&';";
    let style = "text::before { content: '</style>&'; }";
    let page = html! {
        <svg>
            <script>{TrustedHtml(code)}</script>
            <style>"text::before { content: '</style>&'; }"</style>
        </svg>
    };
    let output = render(&page).unwrap();
    assert!(output.as_str().contains("&lt;/script&gt;&amp;"));
    let document = Html::parse_fragment(output.as_str());
    for (name, expected) in [("script", code), ("style", style)] {
        let selector = Selector::parse(name).unwrap();
        let element = document.select(&selector).next().unwrap();
        assert_eq!(&*element.value().name.ns, "http://www.w3.org/2000/svg");
        assert_eq!(element.text().collect::<String>(), expected);
    }
}

#[test]
fn raw_text_rejects_tokenizer_sequences_across_fragment_boundaries() {
    for value in ["</script>", "</ScRiPt>", "<!--", "<script", "\0"] {
        for boundary in 0..=value.len() {
            let first = TrustedHtml(&value[..boundary]);
            let second = TrustedHtml(&value[boundary..]);
            let page = html! { <script>{first}{second}</script> };
            let error = if value.contains('\0') {
                RenderError::NullCharacter
            } else {
                RenderError::InvalidRawText
            };
            assert_eq!(render(&page), Err(error), "{value:?} split at {boundary}");
        }
    }
    for boundary in 0..=8 {
        let value = "</STYLE>";
        let first = TrustedHtml(&value[..boundary]);
        let second = TrustedHtml(&value[boundary..]);
        assert_eq!(
            render(&html! { <style>{first}{second}</style> }),
            Err(RenderError::InvalidRawText)
        );
    }
    assert_eq!(
        render(&html! { <script>"</scr"{TrustedHtml("ipt>")}</script> }),
        Err(RenderError::InvalidRawText)
    );
}
