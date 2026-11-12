use super::{InputBudget, check_input};
use proc_macro2::{Delimiter, Group, TokenStream};
use quote::quote;

fn expand(input: TokenStream) -> syn::Result<TokenStream> {
    super::expand(quote! { (::rstml_ssr) #input })
}

#[test]
fn parser_budgets_reject_oversized_templates() {
    let tokens: TokenStream = (0..16_384).map(|_| quote!(x)).collect();
    assert!(check_input(tokens.clone(), 0, &mut InputBudget::default()).is_ok());
    let error = expand(quote!(#tokens x)).unwrap_err();
    assert!(error.to_string().contains("16384 tokens"));

    let tags: TokenStream = (0..256).map(|_| quote!(<)).collect();
    assert!(check_input(tags.clone(), 0, &mut InputBudget::default()).is_ok());
    let error = expand(quote!(#tags <)).unwrap_err();
    assert!(error.to_string().contains("256 '<' tokens"));

    let mut nested = quote!("text");
    for _ in 0..64 {
        nested = TokenStream::from(proc_macro2::TokenTree::Group(Group::new(
            Delimiter::Brace,
            nested,
        )));
    }
    assert!(check_input(nested.clone(), 0, &mut InputBudget::default()).is_ok());
    let error = expand(quote!({#nested})).unwrap_err();
    assert!(error.to_string().contains("64 levels"));

    let literal = "x".repeat(1_048_576);
    let error = expand(quote!(#literal)).unwrap_err();
    assert!(error.to_string().contains("one MiB"));

    let literal = "x".repeat(524_288);
    let error = expand(quote!(#literal #literal)).unwrap_err();
    assert!(error.to_string().contains("one MiB"));
}

#[test]
fn match_generation_bounds_branch_depth() {
    let arms: TokenStream = (0..64_u32).map(|value| quote!(#value => "text",)).collect();
    assert!(expand(quote!({match 0_u32 { #arms }})).is_ok());
    let error = expand(quote!({match 0_u32 { #arms _ => "extra", }})).unwrap_err();
    assert!(error.to_string().contains("64 arms"));
}
