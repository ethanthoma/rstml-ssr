use proc_macro2::{Ident, Span, TokenStream as Tokens};
use quote::{ToTokens, format_ident, quote};
use syn::ext::IdentExt;
use syn::{
    Expr, FnArg, GenericParam, ItemFn, Pat, PatIdent, Signature, Type, parse_quote,
    visit_mut::{self, VisitMut},
};

#[derive(Clone, Copy)]
enum PropertyKind {
    Value,
    Children,
}

enum PropertyRequirement {
    Required,
    Optional,
    DefaultValue(Box<Expr>),
}

struct ComponentProperty {
    name: Ident,
    binding: PatIdent,
    property_type: Type,
    kind: PropertyKind,
    requirement: PropertyRequirement,
    state_index: Option<usize>,
}

#[derive(Clone, Copy)]
enum ChildrenSlot {
    Absent,
    Present,
}

#[derive(Clone, Copy)]
enum TemplateNamespace {
    Html,
    Foreign,
}

#[derive(Clone, Copy)]
enum ChildrenCallState {
    NotFound,
    Found,
}

pub fn expand(input: Tokens) -> syn::Result<Tokens> {
    super::check_input(input.clone(), 0, &mut super::InputBudget::default())?;
    let function: ItemFn = syn::parse2(input)?;
    validate_signature(&function.sig)?;
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
    let mut lifetimes = Lifetimes {
        generated: Vec::new(),
    };
    let (properties, children_annotation) = parse_properties(&function.sig.inputs, &mut lifetimes)?;
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
    let names: Vec<&Ident> = properties.iter().map(|property| &property.name).collect();
    let bindings: Vec<&PatIdent> = properties
        .iter()
        .map(|property| &property.binding)
        .collect();
    let types: Vec<&Type> = properties
        .iter()
        .map(|property| &property.property_type)
        .collect();
    let state_names: Vec<Ident> = properties
        .iter()
        .filter_map(|property| {
            property
                .state_index
                .map(|index| format_ident!("__RstmlState{index}"))
        })
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
    initial_arguments.extend(
        properties
            .iter()
            .filter(|property| property.state_index.is_some())
            .map(|_| quote!(#runtime::__Missing)),
    );
    let initial_use = generics(&initial_arguments);
    let mut complete_arguments = lifetime_arguments.clone();
    complete_arguments.extend(
        properties
            .iter()
            .filter(|property| property.state_index.is_some())
            .map(|property| {
                let property_type = &property.property_type;
                quote!(#runtime::__Set<#property_type>)
            }),
    );
    let complete_use = generics(&complete_arguments);
    let builder_fields: Vec<Tokens> = properties
        .iter()
        .map(|property| {
            let field_name = &property.name;
            if let Some(state_index) = property.state_index {
                let state_name = &state_names[state_index];
                quote!(#field_name: #state_name)
            } else {
                let property_type = &property.property_type;
                quote!(#field_name: ::core::option::Option<#property_type>)
            }
        })
        .collect();
    let initial_fields: Vec<Tokens> = properties
        .iter()
        .map(|property| {
            let field_name = &property.name;
            if property.state_index.is_some() {
                quote!(#field_name: #runtime::__Missing)
            } else {
                quote!(#field_name: ::core::option::Option::None)
            }
        })
        .collect();
    let built_fields: Vec<Tokens> = properties
        .iter()
        .map(|property| {
            let field_name = &property.name;
            match &property.requirement {
                PropertyRequirement::Required => quote!(#field_name: self.#field_name.0),
                PropertyRequirement::Optional => {
                    quote!(#field_name: ::core::option::Option::unwrap_or_default(self.#field_name))
                }
                PropertyRequirement::DefaultValue(default_value) => {
                    quote!(#field_name: ::core::option::Option::unwrap_or_else(self.#field_name, || (#default_value)))
                }
            }
        })
        .collect();
    let setters = generate_setters(
        &properties,
        &builder_name,
        &lifetime_arguments,
        &builder_arguments,
        &runtime,
    );
    let child_bound = children_annotation.as_ref().map(|_| {
        quote!(where __RstmlChildren: ::core::ops::FnOnce(&mut #runtime::Output) -> #runtime::RenderResult)
    });
    let children_slot = if children_annotation.is_some() {
        ChildrenSlot::Present
    } else {
        ChildrenSlot::Absent
    };
    let annotation_check = children_annotation.map(|annotation| {
        quote!(let _: ::core::marker::PhantomData<#runtime::Children> = ::core::marker::PhantomData::<#annotation>;)
    });
    let output = Ident::new("__rstml_component_output", Span::mixed_site());
    let mut body = *function.block;
    let mut templates = Templates {
        runtime: runtime.clone(),
        output: output.clone(),
        children: children_slot,
        namespace: TemplateNamespace::Html,
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
            #(#builder_fields,)*
            __marker: ::core::marker::PhantomData<#marker>,
        }
        #(#configuration)*
        impl #builder_generics #builder_name #builder_use { #(#setters)* }
        #(#configuration)*
        impl #properties_generics #builder_name #complete_use #child_bound {
            pub fn build(self) -> #properties_name #properties_use {
                #properties_name { #(#built_fields,)* __marker: ::core::marker::PhantomData }
            }
        }
        #(#configuration)*
        impl #name {
            pub fn builder #lifetime_generics() -> #builder_name #initial_use {
                #builder_name { #(#initial_fields,)* __marker: ::core::marker::PhantomData }
            }
            pub fn render #properties_generics(#output: &mut #runtime::Output, properties: #properties_name #properties_use) #return_type #child_bound {
                let #properties_name { #(#bindings,)* __marker: _ } = properties;
                #annotation_check
                #(#statements)*
            }
        }
    })
}

fn validate_signature(signature: &Signature) -> syn::Result<()> {
    let invalid_function_modifier = signature.asyncness.is_some()
        || signature.constness.is_some()
        || !matches!(signature.safety, syn::Safety::Default)
        || signature.abi.is_some()
        || signature.variadic.is_some();
    if invalid_function_modifier {
        return Err(syn::Error::new_spanned(
            signature,
            "components must be synchronous, safe Rust functions",
        ));
    }
    if !matches!(&signature.output, syn::ReturnType::Type(_, return_type) if matches!(return_type.as_ref(), Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "RenderResult")))
    {
        return Err(syn::Error::new_spanned(
            signature,
            "components must return RenderResult",
        ));
    }
    if signature.inputs.len() > 16 {
        return Err(syn::Error::new_spanned(
            &signature.inputs,
            "components accept at most 16 properties",
        ));
    }
    for parameter in &signature.generics.params {
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
            continue;
        }
        return Err(syn::Error::new_spanned(
            parameter,
            "component type and const generics are not supported yet",
        ));
    }
    if signature.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            &signature.generics,
            "component where clauses are not supported yet",
        ));
    }
    Ok(())
}

fn parse_property_requirement(attributes: &[syn::Attribute]) -> syn::Result<PropertyRequirement> {
    let mut requirement = None;
    for attribute in attributes {
        if attribute.path().is_ident("doc") {
            continue;
        }
        if !attribute.path().is_ident("prop") {
            return Err(syn::Error::new_spanned(
                attribute,
                "property annotations only support #[prop(optional)] and #[prop(default = ...)]",
            ));
        }
        if requirement.is_some() {
            return Err(syn::Error::new_spanned(
                attribute,
                "duplicate #[prop(...)] annotation",
            ));
        }
        let parsed = attribute.parse_args_with(|stream: syn::parse::ParseStream<'_>| {
            let key: Ident = stream.parse()?;
            if key == "optional" {
                if !stream.is_empty() {
                    return Err(stream.error("unexpected tokens after `optional`"));
                }
                return Ok(PropertyRequirement::Optional);
            }
            if key == "default" {
                stream.parse::<syn::Token![=]>()?;
                let expression: Expr = stream.parse()?;
                if !stream.is_empty() {
                    return Err(stream.error("unexpected tokens after default expression"));
                }
                return Ok(PropertyRequirement::DefaultValue(Box::new(expression)));
            }
            Err(syn::Error::new_spanned(
                key,
                "property annotations only support #[prop(optional)] and #[prop(default = ...)]",
            ))
        })?;
        requirement = Some(parsed);
    }
    Ok(requirement.unwrap_or(PropertyRequirement::Required))
}

fn parse_properties(
    inputs: &syn::punctuated::Punctuated<FnArg, syn::Token![,]>,
    lifetimes: &mut Lifetimes,
) -> syn::Result<(Vec<ComponentProperty>, Option<Type>)> {
    let mut properties: Vec<ComponentProperty> = Vec::new();
    let mut children_annotation = None;
    let mut required_count = 0_usize;
    for argument in inputs {
        let FnArg::Typed(argument) = argument else {
            return Err(syn::Error::new_spanned(
                argument,
                "components cannot have a self receiver",
            ));
        };
        let requirement = parse_property_requirement(&argument.attrs)?;
        let Pat::Ident(pattern) = argument.pat.as_ref() else {
            return Err(syn::Error::new_spanned(
                &argument.pat,
                "component properties must be named identifiers",
            ));
        };
        if pattern.by_ref.is_some() {
            return Err(syn::Error::new_spanned(
                pattern,
                "component properties cannot use ref bindings or subpatterns",
            ));
        }
        if pattern.subpat.is_some() {
            return Err(syn::Error::new_spanned(
                pattern,
                "component properties cannot use ref bindings or subpatterns",
            ));
        }
        let field_name = pattern.ident.unraw().to_string();
        if field_name == "build" {
            return Err(syn::Error::new_spanned(
                pattern,
                "duplicate or reserved component property name",
            ));
        }
        if field_name.starts_with("__") {
            return Err(syn::Error::new_spanned(
                pattern,
                "duplicate or reserved component property name",
            ));
        }
        if properties
            .iter()
            .any(|existing| existing.name.unraw() == pattern.ident.unraw())
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
        let mut property_type = *argument.ty.clone();
        let is_children = matches!(&property_type, Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "Children"));
        let kind = if is_children {
            if pattern.ident.unraw() != "children" {
                return Err(syn::Error::new_spanned(
                    pattern,
                    "the Children property must be named children",
                ));
            }
            if !matches!(requirement, PropertyRequirement::Required) {
                return Err(syn::Error::new_spanned(
                    pattern,
                    "#[prop(...)] is not supported on Children",
                ));
            }
            children_annotation = Some(property_type.clone());
            property_type = parse_quote!(__RstmlChildren);
            PropertyKind::Children
        } else {
            lifetimes.visit_type_mut(&mut property_type);
            PropertyKind::Value
        };
        let state_index = if matches!(requirement, PropertyRequirement::Required) {
            let index = required_count;
            required_count += 1;
            Some(index)
        } else {
            None
        };
        properties.push(ComponentProperty {
            name: pattern.ident.clone(),
            binding: pattern.clone(),
            property_type,
            kind,
            requirement,
            state_index,
        });
    }
    Ok((properties, children_annotation))
}

fn generate_setters(
    properties: &[ComponentProperty],
    builder_name: &Ident,
    lifetime_arguments: &[Tokens],
    builder_arguments: &[Tokens],
    runtime: &Tokens,
) -> Vec<Tokens> {
    let builder_use = generics(builder_arguments);
    properties
        .iter()
        .map(|property| {
            let field = &property.name;
            let property_type = &property.property_type;
            if let Some(state_index) = property.state_index {
                let mut next = builder_arguments.to_vec();
                let (value_type, method_generics) = match property.kind {
                    PropertyKind::Children => (
                        quote!(__RstmlChildValue),
                        quote!(<__RstmlChildValue: ::core::ops::FnOnce(&mut #runtime::Output) -> #runtime::RenderResult>),
                    ),
                    PropertyKind::Value => (quote!(#property_type), quote!()),
                };
                next[lifetime_arguments.len() + state_index] = quote!(#runtime::__Set<#value_type>);
                let next = generics(&next);
                let assignments = properties.iter().map(|other| {
                    let name = &other.name;
                    if name == field {
                        quote!(#name: #runtime::__Set(value))
                    } else {
                        quote!(#name: self.#name)
                    }
                });
                quote! {
                    pub fn #field #method_generics(self, value: #value_type) -> #builder_name #next {
                        #builder_name { #(#assignments,)* __marker: ::core::marker::PhantomData }
                    }
                }
            } else {
                let assignments = properties.iter().map(|other| {
                    let name = &other.name;
                    if name == field {
                        quote!(#name: ::core::option::Option::Some(value))
                    } else {
                        quote!(#name: self.#name)
                    }
                });
                quote! {
                    pub fn #field(self, value: #property_type) -> #builder_name #builder_use {
                        #builder_name { #(#assignments,)* __marker: ::core::marker::PhantomData }
                    }
                }
            }
        })
        .collect()
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
    fn visit_type_fn_ptr_mut(&mut self, _function_pointer: &mut syn::TypeFnPtr) {}

    fn visit_type_reference_mut(&mut self, reference_type: &mut syn::TypeReference) {
        if reference_type.lifetime.is_none() {
            reference_type.lifetime = Some(self.next());
        }
        visit_mut::visit_type_reference_mut(self, reference_type);
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
    children: ChildrenSlot,
    namespace: TemplateNamespace,
    error: Option<syn::Error>,
}

impl VisitMut for Templates {
    fn visit_expr_call_mut(&mut self, call: &mut syn::ExprCall) {
        if matches!(self.children, ChildrenSlot::Present) {
            if is_children(call) {
                let output = &self.output;
                call.args.push(parse_quote!(#output));
            }
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
            Err(error) => self.record_error(error),
        }
    }
}

impl Templates {
    fn record_error(&mut self, error: syn::Error) {
        match &mut self.error {
            Some(existing) => existing.combine(error),
            None => self.error = Some(error),
        }
    }

    fn template(&mut self, tokens: Tokens) -> syn::Result<Tokens> {
        let (destination, body) = super::destination(tokens);
        let parser_configuration =
            rstml::ParserConfig::new().always_self_closed_elements(super::VOID_ELEMENTS.into());
        let mut nodes = rstml::Parser::new(parser_configuration).parse_simple(body)?;
        self.nodes(&mut nodes, self.namespace, 0)?;
        let output = destination
            .map(|value| quote!(#value))
            .unwrap_or_else(|| self.output.to_token_stream());
        Ok(quote!(#output, #(#nodes)*))
    }

    fn nodes(
        &mut self,
        nodes: &mut [rstml::node::Node],
        namespace: TemplateNamespace,
        depth: u32,
    ) -> syn::Result<()> {
        if depth > super::TEMPLATE_DEPTH_MAX {
            return Err(syn::Error::new(
                Span::call_site(),
                "html! element nesting exceeds 64 levels",
            ));
        }
        for node in nodes {
            match node {
                rstml::node::Node::Element(element) => {
                    let name = element.name().to_string().to_ascii_lowercase();
                    let child_namespace = match namespace {
                        TemplateNamespace::Foreign => TemplateNamespace::Foreign,
                        TemplateNamespace::Html => match name.as_str() {
                            "svg" | "math" => TemplateNamespace::Foreign,
                            _ => TemplateNamespace::Html,
                        },
                    };
                    self.nodes(&mut element.children, child_namespace, depth + 1)?;
                }
                rstml::node::Node::Fragment(fragment) => {
                    self.nodes(&mut fragment.children, namespace, depth + 1)?
                }
                rstml::node::Node::Block(block) => {
                    let mut value: syn::Block = syn::parse2(block.to_token_stream())?;
                    let output = Ident::new("__rstml_child_output", Span::mixed_site());
                    let mut children = ChildCalls {
                        slot: self.children,
                        output: output.clone(),
                        state: ChildrenCallState::NotFound,
                    };
                    children.visit_block_mut(&mut value);
                    if matches!(children.state, ChildrenCallState::Found) {
                        if matches!(namespace, TemplateNamespace::Foreign) {
                            return Err(syn::Error::new_spanned(
                                &*block,
                                "children slots in SVG or MathML are not supported",
                            ));
                        }
                    }
                    let outer_namespace = std::mem::replace(&mut self.namespace, namespace);
                    match children.state {
                        ChildrenCallState::Found => {
                            let outer = std::mem::replace(&mut self.output, output.clone());
                            self.visit_block_mut(&mut value);
                            self.output = outer;
                            let expression: Expr = syn::parse2(quote!(#value))?;
                            let rendered = super::content_expression(expression, 0, &self.runtime)?;
                            let runtime = &self.runtime;
                            *block = rstml::node::NodeBlock::ValidBlock(syn::parse2(
                                quote!({|#output| #runtime::Render::render(&(#rendered), #output)}),
                            )?);
                        }
                        ChildrenCallState::NotFound => {
                            self.visit_block_mut(&mut value);
                            *block = rstml::node::NodeBlock::ValidBlock(value);
                        }
                    }
                    self.namespace = outer_namespace;
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
    if !call.args.is_empty() {
        return false;
    }
    let Expr::Path(path) = call.func.as_ref() else {
        return false;
    };
    if path.path.segments.len() != 1 {
        return false;
    }
    path.path.segments[0].ident.unraw() == "children"
}

struct ChildCalls {
    slot: ChildrenSlot,
    output: Ident,
    state: ChildrenCallState,
}

impl VisitMut for ChildCalls {
    fn visit_expr_call_mut(&mut self, call: &mut syn::ExprCall) {
        if matches!(self.slot, ChildrenSlot::Present) {
            if is_children(call) {
                let output = &self.output;
                call.args.push(parse_quote!(#output));
                self.state = ChildrenCallState::Found;
            }
        }
        visit_mut::visit_expr_call_mut(self, call);
    }
}
