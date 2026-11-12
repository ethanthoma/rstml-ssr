mod fixtures;

use fixtures::{Assets, PageData, maud_fixture, rstml_fixture};
use rstml_ssr::{html, render};
use scraper::{Html, Selector};

#[test]
fn serialized_json_cannot_escape_attributes_or_script_elements() {
    for payload in [
        "x onmouseover=alert(1) y",
        "\"><img src=x onerror=alert(1)>",
        "</script><script>alert(1)</script>",
        "<!--<script>é🦀&\"'",
        "first\r\nsecond",
    ] {
        let value = serde_json::json!({"name": payload});
        let serialized = value.to_string();
        let script_json = serialized.replace('<', "\\u003c");
        let page = html! {
            <div id="state" data-state={serialized} />
            <script type="application/json">{rstml_ssr::TrustedHtml(&script_json)}</script>
        };
        let markup = render(&page).unwrap();
        let document = Html::parse_fragment(markup.as_str());
        let selector = Selector::parse("#state").unwrap();
        let element = document.select(&selector).next().unwrap();
        assert_eq!(element.value().attrs().count(), 2);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(element.value().attr("data-state").unwrap())
                .unwrap(),
            value
        );
        let selector = Selector::parse("script").unwrap();
        let scripts = document.select(&selector).collect::<Vec<_>>();
        assert_eq!(scripts.len(), 1);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&scripts[0].text().collect::<String>())
                .unwrap(),
            value
        );
        assert_eq!(document.select(&Selector::parse("img").unwrap()).count(), 0);
    }
}

#[test]
fn contact_form_matches_maud_dom_and_decoded_datastar_values() {
    let expected = maud_fixture::contact_form().into_string();
    let actual = render(&rstml_fixture::contact_form())
        .unwrap()
        .into_string();
    assert_eq!(
        Html::parse_fragment(&actual).html(),
        Html::parse_fragment(&expected).html()
    );
    let document = Html::parse_fragment(&actual);
    let form = document
        .select(&Selector::parse("form").unwrap())
        .next()
        .unwrap();
    assert_eq!(
        form.value().attr("data-on:submit"),
        Some("@post('/contact', {contentType: 'form'})")
    );
    assert_eq!(form.value().attr("data-attr:aria-busy"), Some("$fetching"));
    assert_eq!(form.value().attr("data-indicator:fetching"), Some(""));
}

#[test]
fn shared_page_shell_matches_maud_dom() {
    let schema = r#"<script type="application/ld+json">{"@context":"https://schema.org","@type":"Organization","name":"Matter State Research"}</script>"#;
    for url in [
        "https://matterstateresearch.com/",
        "https://matterstateresearch.com/contact",
        "https://matterstateresearch.com/needle",
        "https://matterstateresearch.com/reports/test",
    ] {
        for enabled in [false, true] {
            let assets = Assets {
                development: enabled,
            };
            let meta = PageData {
                title: "Page \"<&> é",
                description: "</title><script>alert('injection')</script>",
                drafts: enabled,
                url,
                schema: enabled.then_some(schema),
                navigation_path: None,
                styles: &["css/contact.css"],
            };
            let expected =
                maud_fixture::page(&assets, meta, maud_fixture::contact_form()).into_string();
            let actual = render(&rstml_fixture::page(
                &assets,
                meta,
                rstml_fixture::contact_form(),
            ))
            .unwrap()
            .into_string();
            assert_eq!(
                Html::parse_document(&actual).html(),
                Html::parse_document(&expected).html(),
                "{url} enabled={enabled}"
            );
            assert!(!actual.contains("<script>alert('injection')</script>"));
        }
    }
}

#[test]
fn generated_text_and_attributes_match_maud_after_html_parsing() {
    let alphabet = [
        "", "&", "<", ">", "\"", "'", "é", "🦀", "\n", "\t", "&quot;",
    ];
    for first in alphabet {
        for second in alphabet {
            for third in alphabet {
                let input = format!("{first}{second}{third}");
                let expected = maud::html! { div data-on:submit=(input) { (input) } }.into_string();
                let actual = render(&html! { <div data-on:submit={input}>{input}</div> })
                    .unwrap()
                    .into_string();
                let document = Html::parse_fragment(&actual);
                assert_eq!(document.html(), Html::parse_fragment(&expected).html());
                let element = document
                    .select(&Selector::parse("div").unwrap())
                    .next()
                    .unwrap();
                assert_eq!(
                    element.value().attr("data-on:submit"),
                    Some(format!("{first}{second}{third}").as_str())
                );
                assert_eq!(
                    element.text().collect::<String>(),
                    format!("{first}{second}{third}")
                );
            }
        }
    }
}

#[test]
fn multiline_fragments_survive_the_website_patch_framing() {
    for text in [
        "",
        "first\nsecond",
        "\n",
        "\n\n",
        "é\n<&>\n",
        "data: fake\n\nevent: fake\n",
        "first\r\nsecond\rthird",
    ] {
        let markup = render(&html! {
            <p id="notice">{text}</p>
            {text}
        })
        .unwrap()
        .into_string();
        let mut data = String::from("selector #notice\nmode replace\n");
        for line in markup.split('\n') {
            data.push_str("elements ");
            data.push_str(line);
            data.push('\n');
        }
        let data = data.trim_end_matches('\n');
        let frame = format!(
            "event: datastar-patch-elements\n{}\n",
            data.split('\n')
                .map(|line| format!("data: {line}\n"))
                .collect::<String>()
        );
        let decoded = frame
            .lines()
            .filter_map(|line| line.strip_prefix("data: elements "))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(decoded, markup);
    }
}

#[test]
fn serialization_differences_are_explicit_and_dom_equivalent() {
    let expected = maud::html! { p data-indicator:fetching { "\"" } }.into_string();
    let actual = render(&html! { <p data-indicator:fetching>"\""</p> })
        .unwrap()
        .into_string();
    assert_eq!(expected, "<p data-indicator:fetching>&quot;</p>");
    assert_eq!(actual, "<p data-indicator:fetching=\"\">\"</p>");
    assert_eq!(
        Html::parse_fragment(&actual).html(),
        Html::parse_fragment(&expected).html()
    );
}

#[test]
fn carriage_returns_are_preserved_after_html_parsing() {
    let expression = "a\r\nb\rc";
    let literal = html! { <p data-on:submit="a\r\nb\rc">"a\r\nb\rc"</p> };
    let dynamic = html! { <p data-on:submit={expression}>{expression}</p> };
    let markup = render(&dynamic).unwrap().into_string();
    assert_eq!(render(&literal).unwrap().as_str(), markup);
    assert!(!markup.contains('\r'));
    let document = Html::parse_fragment(&markup);
    let element = document
        .select(&Selector::parse("p").unwrap())
        .next()
        .unwrap();
    assert_eq!(element.value().attr("data-on:submit"), Some(expression));
    assert_eq!(element.text().collect::<String>(), expression);
    let maud = maud::html! { p data-on:submit=(expression) { (expression) } }.into_string();
    let document = Html::parse_fragment(&maud);
    let element = document
        .select(&Selector::parse("p").unwrap())
        .next()
        .unwrap();
    assert_eq!(element.value().attr("data-on:submit"), Some("a\nb\nc"));
}
