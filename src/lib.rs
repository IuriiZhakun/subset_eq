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
/// Supports: `ignore(field1, field2), method = "name"`
struct Args {
    ignored: Vec<Ident>,
    method: Option<Ident>,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut ignored = Vec::new();
        let mut method = None;

        // Parse comma-separated expressions like `ignore(a, b), method = "foo"`
        let punct = Punctuated::<Expr, Comma>::parse_terminated(input)?;
        for expr in punct {
            match expr {
                // ignore(a, b)
                Expr::Call(call) => {
                    if let Expr::Path(func_path) = *call.func {
                        if func_path.path.is_ident("ignore") {
                            for arg in call.args.iter() {
                                if let Expr::Path(p) = arg {
                                    if let Some(id) = p.path.get_ident() {
                                        ignored.push(id.clone());
                                    } else {
                                        return Err(Error::new(
                                            p.span(),
                                            "Expected identifier in ignore(...)",
                                        ));
                                    }
                                } else {
                                    return Err(Error::new(
                                        arg.span(),
                                        "Expected identifier in ignore(...)",
                                    ));
                                }
                            }
                        } else {
                            return Err(Error::new(func_path.span(), "Expected 'ignore(...)'"));
                        }
                    } else {
                        return Err(Error::new(call.func.span(), "Expected path in ignore(...)"));
                    }
                }
                // method = "foo"
                Expr::Assign(assign) => {
                    // left must be path "method"
                    if let Expr::Path(lp) = *assign.left {
                        if let Some(ident) = lp.path.get_ident() {
                            if ident == "method" {
                                // right must be a string literal
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
                                    "Expected 'method' on left side of assignment",
                                ));
                            }
                        } else {
                            return Err(Error::new(
                                lp.span(),
                                "Expected identifier on left side of assignment",
                            ));
                        }
                    } else {
                        return Err(Error::new(
                            assign.left.span(),
                            "Expected method = \"...\" syntax",
                        ));
                    }
                }
                other => {
                    return Err(Error::new(
                        other.span(),
                        "Unsupported argument; expected ignore(...) or method = \"...\"",
                    ));
                }
            }
        }

        Ok(Args { ignored, method })
    }
}

#[proc_macro_attribute]
pub fn subset_eq(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the target item and the attribute arguments.
    let input = parse_macro_input!(item as DeriveInput);
    let args = parse_macro_input!(attr as Args);

    let method_name = args
        .method
        .unwrap_or_else(|| format_ident!("eq_subset_ignoring"));

    let struct_name = &input.ident;

    // Build the list of fields to compare: all named fields except those ignored.
    let fields_to_compare = match &input.data {
        Data::Struct(ds) => match &ds.fields {
            Fields::Named(fnamed) => fnamed
                .named
                .iter()
                .filter_map(|f| {
                    let ident = f.ident.as_ref().unwrap();
                    if args.ignored.iter().any(|i| i == ident) {
                        None
                    } else {
                        Some(ident.clone())
                    }
                })
                .collect::<Vec<_>>(),
            other => {
                return Error::new(other.span(), "subset_eq only supports named-field structs")
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
            "No fields left to compare after ignoring specified ones",
        )
        .to_compile_error()
        .into();
    }

    // Generate tuple comparisons of references to the kept fields.
    let self_tuple = quote! { ( #( &self.#fields_to_compare, )* ) };
    let other_tuple = quote! { ( #( &other.#fields_to_compare, )* ) };

    let expanded = quote! {
        #input

        impl #struct_name {
            /// Subset equality method ignoring the specified fields.
            pub fn #method_name(&self, other: &Self) -> bool {
                #self_tuple == #other_tuple
            }
        }
    };

    expanded.into()
}
