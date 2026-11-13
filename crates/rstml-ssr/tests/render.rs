use rstml_ssr::{Output, Render, RenderError, TrustedHtml, component, render, render_with_limit};

#[test]
fn generated_unicode_escaping_and_fragment_boundaries() {
    // Every valid Unicode scalar, in bounded batches, including all ASCII hazards.
    for batch in 0..4352_u32 {
        let input: String = (batch * 256..(batch + 1) * 256)
            .filter_map(char::from_u32)
            .filter(|character| *character != '\0')
            .collect();
        let expected = input
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('\r', "&#13;");
        assert_eq!(render(&input).unwrap().as_str(), expected);
        let pieces = component(|output| {
            for character in input.chars() {
                let mut bytes = [0; 4];
                output.text(character.encode_utf8(&mut bytes))?;
            }
            Ok(())
        });
        assert_eq!(render(&pieces).unwrap().as_str(), expected);
        let attribute = component(|output| output.attribute("data-on:submit", &input));
        assert_eq!(
            render(&attribute).unwrap().as_str(),
            format!(" data-on:submit=\"{}\"", expected.replace('"', "&quot;"))
        );
    }
}

#[test]
fn omission_composition_and_numeric_contracts() {
    let child = component(|output| {
        output.trusted(TrustedHtml("<span>"))?;
        "<&>".render(output)?;
        output.trusted(TrustedHtml("</span>"))
    });
    let owned = render(&child).unwrap();
    let parent = component(|output| {
        output.trusted(TrustedHtml("<input"))?;
        output.attribute("disabled", &false)?;
        output.attribute("required", &true)?;
        output.attribute("missing", &None::<String>)?;
        output.attribute("empty", &Some(""))?;
        output.attribute("count", &42_u32)?;
        output.trusted(TrustedHtml(">"))?;
        child.render(output)?;
        owned.render(output)?;
        i128::MIN.render(output)?;
        f64::MAX.render(output)
    });
    let actual = render(&parent).unwrap();
    assert_eq!(
        actual.as_str(),
        format!(
            "<input required=\"\" empty=\"\" count=\"42\"><span>&lt;&amp;&gt;</span><span>&lt;&amp;&gt;</span>{}{}",
            i128::MIN,
            f64::MAX
        )
    );
}

#[test]
fn byte_limits_include_escaping_and_nested_output() {
    let input = "é<&\"";
    let escaped = render(&input).unwrap();
    for limit in 0..32 {
        let result = render_with_limit(&input, limit);
        if (limit as usize) < escaped.as_str().len() {
            assert_eq!(result, Err(RenderError::OutputLimit));
        } else {
            assert_eq!(result.unwrap(), escaped);
        }
    }
    let nested = component(|output| {
        escaped.render(output)?;
        escaped.render(output)
    });
    assert_eq!(
        render_with_limit(&nested, 2 * escaped.as_str().len() as u32 - 1),
        Err(RenderError::OutputLimit)
    );
    assert_eq!(render_with_limit(&"", 0).unwrap().as_str(), "");
    assert_eq!(
        render_with_limit(&u128::MAX, 1),
        Err(RenderError::OutputLimit)
    );
}

#[test]
fn invalid_attribute_names_fail_before_writing() {
    for name in ["", "x=\"bad", "x y", "x>y", "x\0y", "é"] {
        let mut output = Output::new(32);
        assert_eq!(
            output.attribute(name, &"value"),
            Err(RenderError::InvalidAttributeName)
        );
    }
}

#[test]
fn nul_is_rejected_in_text_and_attributes() {
    assert_eq!(render(&"before\0after"), Err(RenderError::NullCharacter));
    let attribute = component(|output| output.attribute("data-on:submit", &"before\0after"));
    assert_eq!(render(&attribute), Err(RenderError::NullCharacter));
}

#[test]
fn unsized_values_render_without_extra_references() {
    use rstml_ssr::Attribute;

    assert_eq!(render("<&>").unwrap().as_str(), "&lt;&amp;&gt;");
    assert_eq!(render_with_limit("é", 1), Err(RenderError::OutputLimit));

    let text = String::from("value");
    let content: &dyn Render = &text;
    assert_eq!(render(content).unwrap().as_str(), "value");
    let attribute: &dyn Attribute = &text;
    let page = component(|output| {
        output.attribute("title", "<&>")?;
        output.attribute("data-value", attribute)
    });
    assert_eq!(
        render(&page).unwrap().as_str(),
        " title=\"&lt;&amp;&gt;\" data-value=\"value\""
    );
}
