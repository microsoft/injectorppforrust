use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, Token, Type, parenthesized, punctuated::Punctuated};

/// Parsed input for the simplified `fn` syntax of `func!`.
struct FuncInput {
    func_expr: Expr,
    arg_types: Vec<Type>,
    return_type: Option<Type>,
    is_unsafe: bool,
    extern_abi: Option<String>,
}

impl Parse for FuncInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Strip optional "func_info:" prefix
        if input.peek(syn::Ident) {
            let fork = input.fork();
            let ident: syn::Ident = fork.parse()?;
            if ident == "func_info" && fork.peek(Token![:]) {
                // Consume "func_info:"
                let _: syn::Ident = input.parse()?;
                let _: Token![:] = input.parse()?;
            }
        }

        // Parse optional "unsafe"
        let is_unsafe = input.peek(Token![unsafe]);
        if is_unsafe {
            let _: Token![unsafe] = input.parse()?;
            // Skip optional empty braces "{}" used in some macro arms
            if input.peek(syn::token::Brace) {
                let content;
                syn::braced!(content in input);
                let _ = content;
            }
        }

        // Parse optional extern "ABI"
        let extern_abi = if input.peek(Token![extern]) {
            let _: Token![extern] = input.parse()?;
            let abi: syn::LitStr = input.parse()?;
            Some(abi.value())
        } else {
            None
        };

        // Parse "fn"
        let _: Token![fn] = input.parse()?;

        // Parse "( func_expr )"
        let func_content;
        parenthesized!(func_content in input);
        let func_expr: Expr = func_content.parse()?;

        // Parse "( arg_types )"
        let types_content;
        parenthesized!(types_content in input);
        let arg_types: Punctuated<Type, Token![,]> =
            Punctuated::parse_terminated(&types_content)?;

        // Parse optional "-> return_type"
        let return_type = if input.peek(Token![->]) {
            let _: Token![->] = input.parse()?;
            Some(input.parse::<Type>()?)
        } else {
            None
        };

        Ok(FuncInput {
            func_expr,
            arg_types: arg_types.into_iter().collect(),
            return_type,
            is_unsafe,
            extern_abi,
        })
    }
}

/// Check if a type is a bare reference (& without explicit lifetime).
fn is_bare_reference(ty: &Type) -> bool {
    match ty {
        Type::Reference(ref_type) => ref_type.lifetime.is_none(),
        _ => false,
    }
}

/// Proc macro that implements the simplified `fn` syntax for `func!` with
/// automatic compile-time lifetime safety checks.
///
/// When the return type is a bare reference (e.g., `&str`, `&[u8]`), this macro
/// generates an invariance-based check that detects lifetime mismatches at compile
/// time. This catches the issue #73 pattern where `fn(&str) -> &'static str` is
/// incorrectly specified as `fn(&str) -> &str`.
///
/// For functions with genuinely linked lifetimes (e.g., `fn(&str) -> &str` where
/// the output lifetime matches the input), use an explicit lifetime annotation:
/// `func!(fn (f)(&str) -> &'_ str)`.
#[proc_macro]
pub fn func_checked(input: TokenStream) -> TokenStream {
    let parsed = match syn::parse::<FuncInput>(input) {
        Ok(parsed) => parsed,
        Err(err) => return err.to_compile_error().into(),
    };

    let func_expr = &parsed.func_expr;
    let arg_types = &parsed.arg_types;

    // Build the function pointer type
    let fn_type = match (&parsed.return_type, parsed.is_unsafe, &parsed.extern_abi) {
        (Some(ret), false, None) => quote! { fn(#(#arg_types),*) -> #ret },
        (None, false, None) => quote! { fn(#(#arg_types),*) },
        (Some(ret), true, None) => quote! { unsafe fn(#(#arg_types),*) -> #ret },
        (None, true, None) => quote! { unsafe fn(#(#arg_types),*) -> () },
        (Some(ret), true, Some(abi)) => {
            let abi_lit = syn::LitStr::new(abi, proc_macro2::Span::call_site());
            quote! { unsafe extern #abi_lit fn(#(#arg_types),*) -> #ret }
        }
        (None, true, Some(abi)) => {
            let abi_lit = syn::LitStr::new(abi, proc_macro2::Span::call_site());
            quote! { unsafe extern #abi_lit fn(#(#arg_types),*) -> () }
        }
        (Some(ret), false, Some(abi)) => {
            let abi_lit = syn::LitStr::new(abi, proc_macro2::Span::call_site());
            quote! { extern #abi_lit fn(#(#arg_types),*) -> #ret }
        }
        (None, false, Some(abi)) => {
            let abi_lit = syn::LitStr::new(abi, proc_macro2::Span::call_site());
            quote! { extern #abi_lit fn(#(#arg_types),*) }
        }
    };

    // Generate the lifetime invariance check for bare reference returns.
    // This only applies to non-unsafe functions (unsafe/extern functions
    // typically don't have Rust lifetime semantics).
    let lifetime_check = if !parsed.is_unsafe {
        if let Some(ref ret) = parsed.return_type {
            if is_bare_reference(ret) {
                quote! {
                    {
                        fn __injpp_check_ret<__R>(
                            _f: fn(#(#arg_types),*) -> __R,
                        ) -> fn(#(#arg_types),*) -> __R {
                            _f
                        }
                        fn __injpp_eq<__T>(_: &mut __T, _: &mut __T) {}
                        let mut __a = __injpp_check_ret(#func_expr);
                        let mut __b: fn(#(#arg_types),*) -> #ret = #func_expr;
                        __injpp_eq(&mut __a, &mut __b);
                    }
                }
            } else {
                quote! {}
            }
        } else {
            quote! {}
        }
    } else {
        quote! {}
    };

    let output = quote! {
        {
            #lifetime_check
            {
                let fn_val: #fn_type = #func_expr;
                let ptr = fn_val as *const ();
                let sig = std::any::type_name_of_val(&fn_val);
                let type_id = std::any::TypeId::of::<#fn_type>();
                unsafe { FuncPtr::new_with_type_id(ptr, sig, type_id) }
            }
        }
    };

    output.into()
}
