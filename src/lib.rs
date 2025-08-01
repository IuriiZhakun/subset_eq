//! `subset_eq` is a procedural attribute macro that lets you specify only the fields
//! to ignore and auto-generate a helper comparing the rest. It does **not** override
//! the normal `PartialEq`/`Eq`; instead you get an explicit subset comparison.
//!
//! ## Example
//! ```rust
//! # use subset_eq::subset_eq; // bring the macro into scope for the doctest (hidden in rendered docs)
//! #[derive(Debug, Clone, PartialEq, Eq)]
//! #[subset_eq(ignore(updated_at, cache_token), method = "eq_ignoring_meta")]
//! struct Item {
//!     id: u64,
//!     name: String,
//!     updated_at: i64,
//!     cache_token: String,
//! }
//! let a = Item { id: 1, name: "A".into(), updated_at: 0, cache_token: "t".into() };
//! let mut b = a.clone();
//! b.updated_at = 5; // ignored
//! assert!(a.eq_ignoring_meta(&b));
//! ```
//!
//! ### Teaching notes / rationale
//! 1. Procedural macros must live in their own crate with `proc-macro = true` because they are compiled for the host and produce code used in the consuming crate. :contentReference[oaicite:0]{index=0}  
//! 2. We parse attribute arguments manually via the `Parse` trait to avoid brittle assumptions about internal AST shapes (e.g., avoiding direct reliance on legacy `MetaList.nested`). :contentReference[oaicite:1]{index=1}  
//! 3. Matching AST nodes directly (`Expr::Path`, `is_ident("ignore")`) instead of stringifying tokens is faster and idiomatic. :contentReference[oaicite:2]{index=2}  
//! 4. Tuple comparison `(&self.f1, &self.f2, ...) == (&other.f1, &other.f2, ...)` reuses each field’s `PartialEq` implementation with zero overhead. :contentReference[oaicite:3]{index=3}  
//! 5. Errors are surfaced early with spans using `syn::Error` so misuse shows clear compile-time diagnostics. :contentReference[oaicite:4]{index=4}  

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    spanned::Spanned,
    token::Comma,
    Data, DeriveInput, Error, Expr, Fields, Ident,
};

/// Parsed attribute arguments for `#[subset_eq(...)]`.
/// Supported components (in any order):
///   - `ignore(field1, field2)`
///   - `method = "custom_name"`
struct Args {
    ignored: Vec<Ident>,
    method: Option<Ident>,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut ignored = Vec::new();
        let mut method = None;

        // Flexible comma-separated list: allows `ignore(a,b), method = "x"` or reversed. :contentReference[oaicite:5]{index=5}
        let items = Punctuated::<Expr, Comma>::parse_terminated(input)?;
        for item in items {
            match item {
                // Handles `ignore(a, b)`
                Expr::Call(call) => {
                    // Expect the function path to be `ignore`
                    if let Expr::Path(func_path) = *call.func {
                        if func_path.path.is_ident("ignore") {
                            for arg in call.args.iter() {
                                if let Expr::Path(p) = arg {
                                    if let Some(id) = p.path.get_ident() {
                                        ignored.push(id.clone());
                                    } else {
                                        return Err(Error::new(
                                            p.span(),
                                            "expected identifier in ignore(...)",
                                        ));
                                    }
                                } else {
                                    return Err(Error::new(
                                        arg.span(),
                                        "expected identifier in ignore(...)",
                                    ));
                                }
                            }
                        } else {
                            return Err(Error::new(func_path.span(), "expected `ignore(...)`"));
                        }
                    } else {
                        return Err(Error::new(call.func.span(), "expected path in ignore(...)"));
                    }
                }
                // Handles `method = "name"`
                Expr::Assign(assign) => {
                    if let Expr::Path(lp) = *assign.left {
                        if let Some(ident) = lp.path.get_ident() {
                            if ident == "method" {
                                if let Expr::Lit(el) = *assign.right {
                                    if let syn::Lit::Str(ls) = el.lit {
                                        method = Some(format_ident!("{}", ls.value()));
                                    } else {
                                        return Err(Error::new(
                                            el.lit.span(),
                                            "method value must be a string literal",
                                        ));
                                    }
                                } else {
                                    return Err(Error::new(
                                        assign.right.span(),
                                        "method value must be a string literal",
                                    ));
                                }
                            } else {
                                return Err(Error::new(
                                    ident.span(),
                                    "expected `method` on left-hand side",
                                ));
                            }
                        } else {
                            return Err(Error::new(
                                lp.span(),
                                "expected identifier on left-hand side",
                            ));
                        }
                    } else {
                        return Err(Error::new(
                            assign.left.span(),
                            "expected `method = \"...\"` syntax",
                        ));
                    }
                }
                other => {
                    return Err(Error::new(
                        other.span(),
                        "unsupported argument; use `ignore(...)` or `method = \"...\"`",
                    ));
                }
            }
        }

        Ok(Args { ignored, method })
    }
}

/// The procedural attribute macro entry point.  
/// Usage example:
/// `#[subset_eq(ignore(updated_at), method = "eq_no_meta")]`
#[proc_macro_attribute]
pub fn subset_eq(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the item the attribute is applied to (should be a struct).
    let input = parse_macro_input!(item as DeriveInput);
    // Parse our custom arguments.
    let Args { ignored, method } = parse_macro_input!(attr as Args);

    // Determine generated method name, fallback if unspecified.
    let method_name = method.unwrap_or_else(|| format_ident!("eq_subset_ignoring"));
    let struct_name = &input.ident;

    // Collect all named fields that are not ignored.
    let fields_to_compare = match &input.data {
        Data::Struct(ds) => match &ds.fields {
            Fields::Named(named) => named
                .named
                .iter()
                .filter_map(|f| {
                    let id = f.ident.as_ref().unwrap();
                    if ignored.iter().any(|x| x == id) {
                        None
                    } else {
                        Some(id.clone())
                    }
                })
                .collect::<Vec<_>>(),
            other => {
                return Error::new(
                    other.span(),
                    "subset_eq only supports structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return Error::new(input.span(), "subset_eq can only be applied to structs")
                .to_compile_error()
                .into();
        }
    };

    if fields_to_compare.is_empty() {
        return Error::new(
            input.span(),
            "no fields left to compare after ignoring specified ones",
        )
        .to_compile_error()
        .into();
    }

    // Build tuple comparison to leverage existing `PartialEq` implementations.
    let self_tuple = quote! { ( #( &self.#fields_to_compare, )* ) };
    let other_tuple = quote! { ( #( &other.#fields_to_compare, )* ) };

    // Emit original struct plus the subset equality helper method.
    let expanded = quote! {
        #input

        impl #struct_name {
            /// Generated subset equality method ignoring the specified fields.
            pub fn #method_name(&self, other: &Self) -> bool {
                #self_tuple == #other_tuple
            }
        }
    };

    expanded.into()
}
