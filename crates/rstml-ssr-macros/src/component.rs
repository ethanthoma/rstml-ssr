use proc_macro2::{Ident, Span, TokenStream as Tokens};
use quote::{ToTokens, format_ident, quote};
use syn::ext::IdentExt;
use syn::{
    Expr, FnArg, GenericParam, ItemFn, Pat, Type, parse_quote,
    visit_mut::{self, VisitMut},
};

pub fn expand(input: Tokens) -> syn::Result<Tokens> {
    super::check_input(input.clone(), 0, &mut super::InputBudget::default())?;
    let function: ItemFn = syn::parse2(input)?;
    if function.sig.asyncness.is_some()
        || function.sig.constness.is_some()
        || !matches!(function.sig.safety, syn::Safety::Default)
        || function.sig.abi.is_some()
        || function.sig.variadic.is_some()
    {
        return Err(syn::Error::new_spanned(
            &function.sig,
            "components must be synchronous, safe Rust functions",
        ));
    }
    if !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if matches!(ty.as_ref(), Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "RenderResult")))
    {
        return Err(syn::Error::new_spanned(
            &function.sig,
            "components must return RenderResult",
        ));
    }
    if function.sig.inputs.len() > 16 {
        return Err(syn::Error::new_spanned(
            &function.sig.inputs,
            "components accept at most 16 properties",
        ));
    }
    for parameter in &function.sig.generics.params {
        if let GenericParam::Lifetime(lifetime) = parameter {
            if lifetime
                .lifetime
                .ident
                .unraw()
                .to_string()
                .starts_with("__rstml")
            {
                return Err(syn::Error::new_spanned(
                    parameter,
                    "the __rstml lifetime prefix is reserved",
                ));
            }
        }
        if !matches!(parameter, GenericParam::Lifetime(_)) {
            return Err(syn::Error::new_spanned(
                parameter,
                "component type and const generics are not supported yet",
            ));
        }
    }
    if function.sig.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            &function.sig.generics,
            "component where clauses are not supported yet",
        ));
    }
    let runtime = runtime_path()?;
    let name = component_name(&function.sig.ident)?;
    let properties_name = format_ident!("{name}Props");
    let builder_name = format_ident!("{name}PropsBuilder");
    let visibility = &function.vis;
    let return_type = &function.sig.output;
    let attributes = &function.attrs;
    let configuration: Vec<_> = attributes
        .iter()
        .filter(|attribute| {
            attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr")
        })
        .collect();
    let mut names = Vec::new();
    let mut bindings = Vec::new();
    let mut types = Vec::new();
    let mut children_annotation = None;
    let mut lifetimes = Lifetimes {
        generated: Vec::new(),
    };
    for argument in &function.sig.inputs {
        let FnArg::Typed(argument) = argument else {
            return Err(syn::Error::new_spanned(
                argument,
                "components cannot have a self receiver",
            ));
        };
        if argument
            .attrs
            .iter()
            .any(|attribute| !attribute.path().is_ident("doc"))
        {
            return Err(syn::Error::new_spanned(
                argument,
                "property annotations are not supported yet",
            ));
        }
        let Pat::Ident(pattern) = argument.pat.as_ref() else {
            return Err(syn::Error::new_spanned(
                &argument.pat,
                "component properties must be named identifiers",
            ));
        };
        if pattern.by_ref.is_some() || pattern.subpat.is_some() {
            return Err(syn::Error::new_spanned(
                pattern,
                "component properties cannot use ref bindings or subpatterns",
            ));
        }
        let field_name = pattern.ident.unraw().to_string();
        if field_name == "build"
            || field_name.starts_with("__")
            || names
                .iter()
                .any(|name: &Ident| name.unraw() == pattern.ident.unraw())
        {
            return Err(syn::Error::new_spanned(
                pattern,
                "duplicate or reserved component property name",
            ));
        }
        if matches!(argument.ty.as_ref(), Type::ImplTrait(_)) {
            return Err(syn::Error::new_spanned(
                &argument.ty,
                "component properties require concrete types",
            ));
        }
        let mut ty = *argument.ty.clone();
        let is_children = matches!(&ty, Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "Children"));
        if is_children {
            if pattern.ident.unraw() != "children" {
                return Err(syn::Error::new_spanned(
                    pattern,
                    "the Children property must be named children",
                ));
            }
            children_annotation = Some(ty.clone());
            ty = parse_quote!(__RstmlChildren);
        } else {
            lifetimes.visit_type_mut(&mut ty);
        }
        names.push(pattern.ident.clone());
        bindings.push(pattern.clone());
        types.push(ty);
    }
    let mut parameters: Vec<Tokens> = function
        .sig
        .generics
        .params
        .iter()
        .map(ToTokens::to_token_stream)
        .collect();
    let mut arguments: Vec<Tokens> = function
        .sig
        .generics
        .lifetimes()
        .map(|value| value.lifetime.to_token_stream())
        .collect();
    if parameters.len() + lifetimes.generated.len() > 64 {
        return Err(syn::Error::new_spanned(
            &function.sig,
            "components accept at most 64 borrowed lifetimes",
        ));
    }
    parameters.extend(lifetimes.generated.iter().map(ToTokens::to_token_stream));
    arguments.extend(lifetimes.generated.iter().map(ToTokens::to_token_stream));
    let lifetime_parameters = parameters.clone();
    let lifetime_arguments = arguments.clone();
    let marker = if arguments.is_empty() {
        quote!(())
    } else {
        quote!((#(&#arguments (),)*))
    };
    if children_annotation.is_some() {
        parameters.push(quote!(__RstmlChildren));
        arguments.push(quote!(__RstmlChildren));
    }
    let state_names: Vec<Ident> = (0..names.len())
        .map(|index| format_ident!("__RstmlState{index}"))
        .collect();
    let states: Vec<Tokens> = state_names.iter().map(ToTokens::to_token_stream).collect();
    let mut builder_parameters = lifetime_parameters.clone();
    builder_parameters.extend(states.clone());
    let mut builder_arguments = lifetime_arguments.clone();
    builder_arguments.extend(states);
    let builder_generics = generics(&builder_parameters);
    let builder_use = generics(&builder_arguments);
    let properties_generics = generics(&parameters);
    let properties_use = generics(&arguments);
    let mut initial_arguments = lifetime_arguments.clone();
    initial_arguments.extend(names.iter().map(|_| quote!(#runtime::__Missing)));
    let initial_use = generics(&initial_arguments);
    let mut complete_arguments = lifetime_arguments.clone();
    complete_arguments.extend(types.iter().map(|ty| quote!(#runtime::__Set<#ty>)));
    let complete_use = generics(&complete_arguments);
    let setters = names.iter().enumerate().map(|(index, field)| {
        let ty = &types[index];
        let mut next = builder_arguments.clone();
        let value_type = if field.unraw() == "children" {quote!(__RstmlChildValue)} else {quote!(#ty)};
        next[lifetime_arguments.len() + index] = quote!(#runtime::__Set<#value_type>);
        let next = generics(&next);
        let method_generics = if field.unraw() == "children" {quote!(<__RstmlChildValue: ::core::ops::FnOnce(&mut #runtime::Output) -> #runtime::RenderResult>)} else {quote!()};
        let assignments = names.iter().map(|name| if name == field {quote!(#name: #runtime::__Set(value))} else {quote!(#name: self.#name)});
        quote! {
            pub fn #field #method_generics(self, value: #value_type) -> #builder_name #next {
                #builder_name { #(#assignments,)* __marker: ::core::marker::PhantomData }
            }
        }
    }).collect::<Vec<_>>();
    let child_bound = children_annotation.as_ref().map(|_| quote!(where __RstmlChildren: ::core::ops::FnOnce(&mut #runtime::Output) -> #runtime::RenderResult));
    let annotation_check = children_annotation.map(|annotation| quote!(let _: ::core::marker::PhantomData<#runtime::Children> = ::core::marker::PhantomData::<#annotation>;));
    let output = Ident::new("__rstml_component_output", Span::mixed_site());
    let mut body = *function.block;
    let mut templates = Templates {
        runtime: runtime.clone(),
        output: output.clone(),
        children: annotation_check.is_some(),
        foreign: false,
        error: None,
    };
    templates.visit_block_mut(&mut body);
    if let Some(error) = templates.error {
        return Err(error);
    }
    let lifetime_generics = generics(&lifetime_parameters);
    let statements = &body.stmts;
    Ok(quote! {
        #(#attributes)*
        #visibility struct #name;
        #(#configuration)*
        #[doc(hidden)]
        #visibility struct #properties_name #properties_generics {
            #(#names: #types,)*
            __marker: ::core::marker::PhantomData<#marker>,
        }
        #(#configuration)*
        #[doc(hidden)]
        #visibility struct #builder_name #builder_generics {
            #(#names: #state_names,)*
            __marker: ::core::marker::PhantomData<#marker>,
        }
        #(#configuration)*
        impl #builder_generics #builder_name #builder_use { #(#setters)* }
        #(#configuration)*
        impl #properties_generics #builder_name #complete_use #child_bound {
            pub fn build(self) -> #properties_name #properties_use {
                #properties_name { #(#names: self.#names.0,)* __marker: ::core::marker::PhantomData }
            }
        }
        #(#configuration)*
        impl #name {
            pub fn builder #lifetime_generics() -> #builder_name #initial_use {
                #builder_name { #(#names: #runtime::__Missing,)* __marker: ::core::marker::PhantomData }
            }
            pub fn render #properties_generics(#output: &mut #runtime::Output, properties: #properties_name #properties_use) #return_type #child_bound {
                let #properties_name { #(#bindings,)* __marker: _ } = properties;
                #annotation_check
                #(#statements)*
            }
        }
    })
}

fn generics(parameters: &[Tokens]) -> Tokens {
    if parameters.is_empty() {
        quote!()
    } else {
        quote!(<#(#parameters),*>)
    }
}

fn runtime_path() -> syn::Result<Tokens> {
    match proc_macro_crate::crate_name("rstml-ssr") {
        Ok(proc_macro_crate::FoundCrate::Itself) => Ok(quote!(::rstml_ssr)),
        Ok(proc_macro_crate::FoundCrate::Name(name)) => {
            let path = syn::parse_str::<syn::Path>(&format!("::{name}"))
                .or_else(|_| syn::parse_str::<syn::Path>(&format!("::r#{name}")))?;
            Ok(quote!(#path))
        }
        Err(error) => Err(syn::Error::new(Span::call_site(), error.to_string())),
    }
}

fn component_name(name: &Ident) -> syn::Result<Ident> {
    let source = name.unraw().to_string();
    let mut value = String::new();
    let mut uppercase = true;
    for character in source.chars() {
        if character == '_' {
            uppercase = true;
            continue;
        }
        if uppercase {
            value.extend(character.to_uppercase());
            uppercase = false;
        } else {
            value.push(character);
        }
    }
    let mut identifier = syn::parse_str::<Ident>(&value).map_err(|_| {
        syn::Error::new(
            name.span(),
            "component names must contain a letter or digit",
        )
    })?;
    identifier.set_span(name.span());
    Ok(identifier)
}

struct Lifetimes {
    generated: Vec<syn::Lifetime>,
}
impl Lifetimes {
    fn next(&mut self) -> syn::Lifetime {
        let lifetime = syn::Lifetime::new(
            &format!("'__rstml_{}", self.generated.len()),
            Span::mixed_site(),
        );
        self.generated.push(lifetime.clone());
        lifetime
    }
}
impl VisitMut for Lifetimes {
    fn visit_type_fn_ptr_mut(&mut self, _ty: &mut syn::TypeFnPtr) {}
    fn visit_type_reference_mut(&mut self, ty: &mut syn::TypeReference) {
        if ty.lifetime.is_none() {
            ty.lifetime = Some(self.next());
        }
        visit_mut::visit_type_reference_mut(self, ty);
    }
    fn visit_lifetime_mut(&mut self, lifetime: &mut syn::Lifetime) {
        if lifetime.ident == "_" {
            *lifetime = self.next();
        }
    }
}

struct Templates {
    runtime: Tokens,
    output: Ident,
    children: bool,
    foreign: bool,
    error: Option<syn::Error>,
}
impl VisitMut for Templates {
    fn visit_expr_call_mut(&mut self, call: &mut syn::ExprCall) {
        if self.children && is_children(call) {
            let output = &self.output;
            call.args.push(parse_quote!(#output));
        }
        visit_mut::visit_expr_call_mut(self, call);
    }
    fn visit_macro_mut(&mut self, value: &mut syn::Macro) {
        if value
            .path
            .segments
            .last()
            .is_none_or(|segment| segment.ident != "html")
        {
            return;
        }
        match self.template(value.tokens.clone()) {
            Ok(tokens) => value.tokens = tokens,
            Err(error) => self.error = Some(error),
        }
    }
}
impl Templates {
    fn template(&mut self, tokens: Tokens) -> syn::Result<Tokens> {
        let (destination, body) = super::destination(tokens);
        let config =
            rstml::ParserConfig::new().always_self_closed_elements(super::VOID_ELEMENTS.into());
        let mut nodes = rstml::Parser::new(config).parse_simple(body)?;
        self.nodes(&mut nodes, self.foreign)?;
        let output = destination
            .map(|value| quote!(#value))
            .unwrap_or_else(|| self.output.to_token_stream());
        Ok(quote!(#output, #(#nodes)*))
    }
    fn nodes(&mut self, nodes: &mut [rstml::node::Node], foreign: bool) -> syn::Result<()> {
        for node in nodes {
            match node {
                rstml::node::Node::Element(element) => {
                    let name = element.name().to_string().to_ascii_lowercase();
                    self.nodes(
                        &mut element.children,
                        foreign || matches!(name.as_str(), "svg" | "math"),
                    )?;
                }
                rstml::node::Node::Fragment(fragment) => {
                    self.nodes(&mut fragment.children, foreign)?
                }
                rstml::node::Node::Block(block) => {
                    let mut value: syn::Block = syn::parse2(block.to_token_stream())?;
                    let output = Ident::new("__rstml_child_output", Span::mixed_site());
                    let mut children = ChildCalls {
                        enabled: self.children,
                        output: output.clone(),
                        found: false,
                    };
                    children.visit_block_mut(&mut value);
                    if children.found && foreign {
                        return Err(syn::Error::new_spanned(
                            &*block,
                            "children slots in SVG or MathML are not supported",
                        ));
                    }
                    let outer_foreign = std::mem::replace(&mut self.foreign, foreign);
                    if children.found {
                        let outer = std::mem::replace(&mut self.output, output.clone());
                        self.visit_block_mut(&mut value);
                        self.output = outer;
                        let expression: Expr = syn::parse2(quote!(#value))?;
                        let rendered = super::content_expression(expression, 0, &self.runtime)?;
                        let runtime = &self.runtime;
                        *block = rstml::node::NodeBlock::ValidBlock(syn::parse2(
                            quote!({|#output| #runtime::Render::render(&(#rendered), #output)}),
                        )?);
                    } else {
                        self.visit_block_mut(&mut value);
                        *block = rstml::node::NodeBlock::ValidBlock(value);
                    }
                    self.foreign = outer_foreign;
                }
                rstml::node::Node::Comment(_)
                | rstml::node::Node::Doctype(_)
                | rstml::node::Node::Text(_)
                | rstml::node::Node::RawText(_) => {}
                rstml::node::Node::Custom(_) => {
                    return Err(syn::Error::new(
                        Span::call_site(),
                        "custom template nodes are not supported",
                    ));
                }
            }
        }
        Ok(())
    }
}
fn is_children(call: &syn::ExprCall) -> bool {
    call.args.is_empty()
        && matches!(call.func.as_ref(), Expr::Path(path) if path.path.segments.len() == 1 && path.path.segments[0].ident.unraw() == "children")
}
struct ChildCalls {
    enabled: bool,
    output: Ident,
    found: bool,
}
impl VisitMut for ChildCalls {
    fn visit_expr_call_mut(&mut self, call: &mut syn::ExprCall) {
        if self.enabled && is_children(call) {
            let output = &self.output;
            call.args.push(parse_quote!(#output));
            self.found = true;
        }
        visit_mut::visit_expr_call_mut(self, call);
    }
}
