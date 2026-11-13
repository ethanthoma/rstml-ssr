use proc_macro::TokenStream;
use proc_macro2::{Delimiter, Ident, Span, TokenStream as Tokens, TokenTree};
use quote::{ToTokens, quote, quote_spanned};
use rstml::{
    Parser, ParserConfig,
    node::{Infallible, KeyedAttributeValue, Node, NodeAttribute, NodeElement, NodeName},
};
use syn::{Expr, Lit, Stmt, spanned::Spanned};

const VOID_ELEMENTS: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

enum EscapeContext {
    Text,
    Attribute,
}

#[derive(Clone, Copy)]
enum MarkupContext {
    Html,
    Svg,
    MathMl,
    MathText,
    MathAnnotation,
}

#[proc_macro]
pub fn html(input: TokenStream) -> TokenStream {
    expand(input.into())
        .unwrap_or_else(|error| {
            let diagnostic = error.into_compile_error();
            quote! {{ #diagnostic }}
        })
        .into()
}

#[proc_macro]
pub fn write_html(input: TokenStream) -> TokenStream {
    expand_mode(input.into(), true)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand(input: Tokens) -> syn::Result<Tokens> {
    expand_mode(input, false)
}

fn expand_mode(input: Tokens, writing: bool) -> syn::Result<Tokens> {
    let mut tokens = input.into_iter();
    let Some(TokenTree::Group(runtime)) = tokens.next() else {
        return Err(syn::Error::new(
            Span::call_site(),
            "use the html! macro from the runtime crate",
        ));
    };
    if runtime.delimiter() != Delimiter::Parenthesis {
        return Err(syn::Error::new(
            runtime.span(),
            "expected a runtime crate path",
        ));
    }
    let runtime = runtime.stream();
    let destination = if writing {
        let Some(TokenTree::Group(destination)) = tokens.next() else {
            return Err(syn::Error::new(
                Span::call_site(),
                "expected an output expression",
            ));
        };
        Some(syn::parse2::<Expr>(destination.stream())?)
    } else {
        None
    };
    let input: Tokens = tokens.collect();
    check_input(input.clone(), 0, &mut InputBudget::default())?;
    let config = ParserConfig::new().always_self_closed_elements(VOID_ELEMENTS.into());
    let nodes = Parser::new(config).parse_simple(input)?;
    let mut generator = Generator {
        runtime,
        output: Ident::new("__rstml_output", Span::mixed_site()),
        body: Tokens::new(),
        pending: String::new(),
    };
    generator.nodes(&nodes, MarkupContext::Html)?;
    generator.flush();
    let Generator {
        runtime,
        output,
        body,
        ..
    } = generator;
    let write = quote! {
        #body
        ::core::result::Result::Ok(())
    };
    if let Some(destination) = destination {
        Ok(quote! {{
            (|#output: &mut #runtime::Output| -> #runtime::RenderResult { #write })(&mut *(#destination))
        }})
    } else {
        Ok(quote! {{
            #runtime::render_scope(|#output: &mut #runtime::Output| { #write })
        }})
    }
}

#[derive(Default)]
struct InputBudget {
    token_count: u32,
    tag_count: u32,
    source_size_bytes: u32,
}

fn check_input(input: Tokens, depth: u32, budget: &mut InputBudget) -> syn::Result<()> {
    if depth > 64 {
        return Err(syn::Error::new(
            input.span(),
            "html! token nesting exceeds 64 levels",
        ));
    }
    for token in input {
        budget.token_count += 1;
        if budget.token_count > 16_384 {
            return Err(syn::Error::new(
                token.span(),
                "html! exceeds 16384 tokens; split the template into functions",
            ));
        }
        if let TokenTree::Group(group) = token {
            check_input(group.stream(), depth + 1, budget)?;
            continue;
        }
        let size_bytes = token.to_string().len();
        if size_bytes > 1_048_576 {
            return Err(syn::Error::new(
                token.span(),
                "html! token text exceeds one MiB",
            ));
        }
        budget.source_size_bytes += size_bytes as u32;
        if budget.source_size_bytes > 1_048_576 {
            return Err(syn::Error::new(
                token.span(),
                "html! token text exceeds one MiB",
            ));
        }
        if let TokenTree::Punct(punct) = token {
            if punct.as_char() == '<' {
                budget.tag_count += 1;
                if budget.tag_count > 256 {
                    return Err(syn::Error::new(
                        punct.span(),
                        "html! exceeds 256 '<' tokens; split the template into functions",
                    ));
                }
            }
        }
    }
    Ok(())
}

struct Generator {
    runtime: Tokens,
    output: Ident,
    body: Tokens,
    pending: String,
}

impl Generator {
    fn flush(&mut self) {
        if !self.pending.is_empty() {
            let literal = std::mem::take(&mut self.pending);
            let output = &self.output;
            let runtime = &self.runtime;
            self.body
                .extend(quote! { #output.trusted(#runtime::TrustedHtml(#literal))?; });
        }
    }

    fn nodes(&mut self, nodes: &[Node], namespace: MarkupContext) -> syn::Result<()> {
        for node in nodes {
            match node {
                Node::Element(element) => self.element(element, namespace)?,
                Node::Text(text) => self.pending.push_str(&escaped(
                    &text.value_string(),
                    EscapeContext::Text,
                    text.value.span(),
                )?),
                Node::Fragment(fragment) => self.nodes(&fragment.children, namespace)?,
                Node::Block(block) => {
                    let expression = unblock(syn::parse2(block.to_token_stream())?);
                    if let Expr::Closure(callback) = &expression {
                        if callback.inputs.len() == 1 {
                            self.flush();
                            let output = &self.output;
                            self.body.extend(
                                quote_spanned! {block.span()=> #output.component(#callback)?; },
                            );
                            continue;
                        }
                    }
                    let value = unblock(syn::parse2(content_expression(
                        expression,
                        0,
                        &self.runtime,
                    )?)?);
                    self.flush();
                    let output = &self.output;
                    let runtime = &self.runtime;
                    self.body.extend(quote_spanned! {block.span()=> #runtime::Render::render(&(#value), #output)?; });
                }
                Node::Doctype(doctype) => {
                    if !doctype
                        .value
                        .to_token_stream()
                        .to_string()
                        .eq_ignore_ascii_case("html")
                    {
                        return Err(syn::Error::new_spanned(
                            doctype,
                            "only <!DOCTYPE html> is supported",
                        ));
                    }
                    self.pending.push_str("<!DOCTYPE html>");
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        node,
                        "expected quoted text, a Rust expression, or an HTML element",
                    ));
                }
            }
        }
        Ok(())
    }

    fn element(
        &mut self,
        element: &NodeElement<Infallible>,
        namespace: MarkupContext,
    ) -> syn::Result<()> {
        let name = literal_name(element.name())?;
        let name_lowercase = name.to_ascii_lowercase();
        if matches!(
            name_lowercase.as_str(),
            "xmp" | "iframe" | "noembed" | "noframes" | "plaintext"
        ) {
            return Err(syn::Error::new_spanned(
                element.name(),
                "this element requires audited TrustedHtml in the initial API",
            ));
        }
        let namespace = match (namespace, name_lowercase.as_str()) {
            (MarkupContext::MathText, "mglyph" | "malignmark") => MarkupContext::MathMl,
            (MarkupContext::MathText, "svg") | (MarkupContext::MathAnnotation, "svg") => {
                MarkupContext::Svg
            }
            (MarkupContext::MathText, "math") => MarkupContext::MathMl,
            (MarkupContext::MathText, _) => MarkupContext::Html,
            (MarkupContext::MathAnnotation, _) => MarkupContext::MathMl,
            (MarkupContext::Html, "svg") => MarkupContext::Svg,
            (MarkupContext::Html, "math") => MarkupContext::MathMl,
            (namespace, _) => namespace,
        };
        let is_void = matches!(namespace, MarkupContext::Html)
            && VOID_ELEMENTS.contains(&name_lowercase.as_str());
        if is_void {
            if !element.children.is_empty() {
                return Err(syn::Error::new_spanned(
                    element.name(),
                    "void elements cannot have children",
                ));
            }
        }
        self.pending.push('<');
        self.pending.push_str(&name);
        let mut attribute_spans = std::collections::BTreeMap::new();
        for attribute in element.attributes() {
            if let NodeAttribute::Attribute(attribute) = attribute {
                let name = attribute.key.to_string().to_ascii_lowercase();
                if let Some(first_span) = attribute_spans.insert(name.clone(), attribute.key.span())
                {
                    let mut error = syn::Error::new_spanned(
                        &attribute.key,
                        format!("duplicate attribute `{name}`"),
                    );
                    error.combine(syn::Error::new(first_span, "first attribute defined here"));
                    return Err(error);
                }
            }
            self.attribute(attribute)?;
        }
        self.pending.push('>');
        if matches!(name_lowercase.as_str(), "script" | "style") {
            self.raw_content(&element.children, &name_lowercase, namespace)?;
        } else {
            let child_namespace = match (namespace, name_lowercase.as_str()) {
                (MarkupContext::Svg, "foreignobject" | "desc" | "title") => MarkupContext::Html,
                (MarkupContext::MathMl, "mi" | "mo" | "mn" | "ms" | "mtext") => {
                    MarkupContext::MathText
                }
                (MarkupContext::MathMl, "annotation-xml") => annotation_context(element)?,
                (namespace, _) => namespace,
            };
            self.nodes(&element.children, child_namespace)?;
        }
        if !is_void {
            self.pending.push_str("</");
            self.pending.push_str(&name);
            self.pending.push('>');
        }
        Ok(())
    }

    fn raw_content(
        &mut self,
        nodes: &[Node],
        element: &str,
        namespace: MarkupContext,
    ) -> syn::Result<()> {
        self.flush();
        let body_outer = std::mem::take(&mut self.body);
        self.raw_nodes(nodes, namespace)?;
        self.flush();
        let body = std::mem::replace(&mut self.body, body_outer);
        let output = &self.output;
        let runtime = &self.runtime;
        let element = if element == "script" {
            quote! { #runtime::RawTextElement::Script }
        } else {
            quote! { #runtime::RawTextElement::Style }
        };
        self.body.extend(quote! {
            #output.raw_text(#element, |#output| {
                #body
                ::core::result::Result::Ok(())
            })?;
        });
        Ok(())
    }

    fn raw_nodes(&mut self, nodes: &[Node], namespace: MarkupContext) -> syn::Result<()> {
        for node in nodes {
            match node {
                Node::Text(text) => {
                    let value = text.value_string();
                    if matches!(namespace, MarkupContext::Html) {
                        self.pending.push_str(&value);
                    } else {
                        self.pending.push_str(&escaped(
                            &value,
                            EscapeContext::Text,
                            text.value.span(),
                        )?);
                    }
                }
                Node::Fragment(fragment) => self.raw_nodes(&fragment.children, namespace)?,
                Node::Block(block) => {
                    let value = unblock(syn::parse2(block.to_token_stream())?);
                    self.flush();
                    let output = &self.output;
                    if matches!(namespace, MarkupContext::Html) {
                        self.body.extend(
                            quote_spanned! {block.span()=> #output.raw_value(&(#value))?; },
                        );
                    } else {
                        self.body.extend(
                            quote_spanned! {block.span()=> #output.foreign_value(&(#value))?; },
                        );
                    }
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        node,
                        "script and style content must be quoted text or a TrustedHtml expression",
                    ));
                }
            }
        }
        Ok(())
    }

    fn attribute(&mut self, attribute: &NodeAttribute) -> syn::Result<()> {
        let NodeAttribute::Attribute(attribute) = attribute else {
            return Err(syn::Error::new_spanned(
                attribute,
                "attribute spreading and dynamic names are not supported",
            ));
        };
        let name = literal_name(&attribute.key)?;
        match &attribute.possible_value {
            KeyedAttributeValue::None => self.pending.push_str(&format!(" {name}=\"\"")),
            KeyedAttributeValue::Value(_) => {
                let value = attribute.value().ok_or_else(|| {
                    syn::Error::new_spanned(attribute, "expected a Rust attribute value")
                })?;
                if let Expr::Lit(literal) = value {
                    if let Lit::Str(value) = &literal.lit {
                        self.pending.push_str(&format!(
                            " {name}=\"{}\"",
                            escaped(&value.value(), EscapeContext::Attribute, value.span())?
                        ));
                        return Ok(());
                    }
                    if let Lit::Bool(value) = &literal.lit {
                        if value.value {
                            self.pending.push_str(&format!(" {name}=\"\""));
                        }
                        return Ok(());
                    }
                }
                let name_lowercase = name.to_ascii_lowercase();
                if name_lowercase == "style" || name_lowercase.starts_with("on") {
                    return Err(syn::Error::new_spanned(
                        value,
                        "dynamic style and native event-handler values are not supported",
                    ));
                }
                let expression = unblock(value.clone());
                self.flush();
                let output = &self.output;
                self.body.extend(
                    quote_spanned! {value.span()=> #output.attribute(#name, &(#expression))?; },
                );
            }
            KeyedAttributeValue::Binding(_) => {
                return Err(syn::Error::new_spanned(
                    attribute,
                    "attribute bindings are not supported; use name={value}",
                ));
            }
        }
        Ok(())
    }
}

fn annotation_context(element: &NodeElement<Infallible>) -> syn::Result<MarkupContext> {
    for attribute in element.attributes() {
        if let NodeAttribute::Attribute(attribute) = attribute {
            if attribute.key.to_string().eq_ignore_ascii_case("encoding") {
                if let Some(Expr::Lit(literal)) = attribute.value() {
                    if let Lit::Str(value) = &literal.lit {
                        let encoding = value.value();
                        if encoding.eq_ignore_ascii_case("text/html")
                            || encoding.eq_ignore_ascii_case("application/xhtml+xml")
                        {
                            return Ok(MarkupContext::Html);
                        }
                        return Ok(MarkupContext::MathAnnotation);
                    }
                }
                return Err(syn::Error::new_spanned(
                    attribute,
                    "annotation-xml encoding must be a string literal so its child namespace is known",
                ));
            }
        }
    }
    Ok(MarkupContext::MathAnnotation)
}

fn literal_name(name: &NodeName) -> syn::Result<String> {
    if name.is_block() {
        return Err(syn::Error::new_spanned(
            name,
            "dynamic tag and attribute names are not supported",
        ));
    }
    let value = name.to_string();
    if value.is_empty() {
        return Err(syn::Error::new_spanned(
            name,
            "expected a literal HTML name",
        ));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
    {
        return Err(syn::Error::new_spanned(name, "expected an ASCII HTML name"));
    }
    Ok(value)
}

fn escaped(value: &str, context: EscapeContext, span: Span) -> syn::Result<String> {
    if value.contains('\0') {
        return Err(syn::Error::new(span, "NUL is not representable in HTML"));
    }
    let value = value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\r', "&#13;");
    if matches!(context, EscapeContext::Attribute) {
        Ok(value.replace('"', "&quot;"))
    } else {
        Ok(value)
    }
}

fn unblock(mut expression: Expr) -> Expr {
    while let Expr::Block(block) = &expression {
        if !block.attrs.is_empty() {
            break;
        }
        if block.label.is_some() {
            break;
        }
        if block.block.stmts.len() != 1 {
            break;
        }
        expression = match &block.block.stmts[0] {
            Stmt::Expr(value, None) => value.clone(),
            Stmt::Macro(value) if value.semi_token.is_none() => Expr::Macro(syn::ExprMacro {
                attrs: value.attrs.clone(),
                mac: value.mac.clone(),
            }),
            _ => break,
        };
    }
    expression
}

fn content_expression(expression: Expr, depth: u32, runtime: &Tokens) -> syn::Result<Tokens> {
    if depth > 64 {
        return Err(syn::Error::new_spanned(
            expression,
            "content expression nesting exceeds 64 levels",
        ));
    }
    match unblock(expression) {
        Expr::Block(mut block) => {
            if let Some(Stmt::Expr(value, None)) = block.block.stmts.last_mut() {
                *value = syn::parse2(content_expression(value.clone(), depth + 1, runtime)?)?;
            }
            Ok(quote! { #block })
        }
        Expr::Paren(mut expression) => {
            *expression.expr =
                syn::parse2(content_expression(*expression.expr, depth + 1, runtime)?)?;
            Ok(quote! { #expression })
        }
        Expr::Group(mut expression) => {
            *expression.expr =
                syn::parse2(content_expression(*expression.expr, depth + 1, runtime)?)?;
            Ok(quote! { #expression })
        }
        Expr::If(mut expression) => {
            let other = match expression.else_branch.take() {
                Some((_, other)) => *other,
                None => syn::parse_quote!(()),
            };
            let condition = expression.cond;
            let body = expression.then_branch;
            let body = content_expression(syn::parse2(quote! { #body })?, depth + 1, runtime)?;
            let other = content_expression(other, depth + 1, runtime)?;
            Ok(
                quote! { if #condition { #runtime::__Branch::Left(#body) } else { #runtime::__Branch::Right(#other) } },
            )
        }
        Expr::Closure(mut expression) if expression.inputs.is_empty() => {
            if matches!(expression.output, syn::ReturnType::Default) {
                *expression.body =
                    syn::parse2(content_expression(*expression.body, depth + 1, runtime)?)?;
            }
            Ok(quote! { #runtime::lazy(#expression) })
        }
        Expr::Match(mut expression) => {
            let count = expression.arms.len();
            if count > 64 {
                return Err(syn::Error::new_spanned(
                    expression,
                    "content match expressions support at most 64 arms",
                ));
            }
            for (index, arm) in expression.arms.iter_mut().enumerate() {
                let mut body = content_expression(*arm.body.clone(), depth + 1, runtime)?;
                if index + 1 < count {
                    body = quote! { #runtime::__Branch::Left(#body) };
                }
                for _ in 0..index {
                    body = quote! { #runtime::__Branch::Right(#body) };
                }
                *arm.body = syn::parse2(body)?;
            }
            Ok(quote! { #expression })
        }
        expression => Ok(quote! { #expression }),
    }
}

#[cfg(test)]
mod tests;
