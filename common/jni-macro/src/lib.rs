extern crate proc_macro;

use proc_macro::{TokenStream, TokenTree};
use quote::{format_ident, quote};
use syn::{parse_macro_input, FnArg, ImplItem, ItemImpl, Pat, Type, Visibility};

/// # Example
///
/// ```
/// use jni::*;
///
/// struct Hello;
///
/// #[jni_exports(package = "com.example.hello")]
/// impl Hello {
///     pub fn say_hello(env: JNIEnv, this: JClass) {}
/// }
/// ```
///
/// kotlin:
///
/// ```
/// package com.example.hello
///
/// class Hello {
///     fun sayHello() {
///     }
/// }
/// ````
#[proc_macro_attribute]
pub fn jni_exports(attr: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemImpl);
    let class_name = get_class_name(&input).unwrap();
    let package_name = get_package_name(&attr).unwrap().replace('.', "_");
    let exports = input
        .items
        .iter()
        .map(|item| {
            if let ImplItem::Fn(func) = item {
                if let Visibility::Public(_) = func.vis {
                    let method = func.sig.ident.to_string();
                    let func_name = to_camel_case(&method);
                    let args = func.sig.inputs.clone();
                    let args_ident = get_args_ident(&args);
                    let output = func.sig.output.clone();

                    let func_name_ident = format_ident!("{}", method);
                    let class_name_ident = format_ident!("{}", class_name);
                    let jni_name =
                        format_ident!("Java_{}_{}_{}", package_name, class_name, func_name);

                    return Some(quote! {
                        #[no_mangle]
                        #[allow(unused_mut)]
                        pub extern "system" fn #jni_name(#args) #output {
                            #class_name_ident::#func_name_ident(#(#args_ident)*)
                        }
                    });
                }
            }

            None
        })
        .filter(|item| item.is_some())
        .map(|item| item.unwrap());

    TokenStream::from(quote! {
        #input

        #(#exports)*
    })
}

fn get_package_name(attr: &TokenStream) -> Option<String> {
    let mut iter = attr.clone().into_iter();
    if let Some(TokenTree::Ident(ident)) = iter.next() {
        if let Some(TokenTree::Punct(punct)) = iter.next() {
            if let Some(TokenTree::Literal(literal)) = iter.next() {
                if ident.to_string().as_str() == "package" {
                    if punct.to_string().as_str() == "=" {
                        return Some(literal.to_string().replace("\"", ""));
                    }
                }
            }
        }
    }

    None
}

fn get_class_name(item: &ItemImpl) -> Option<String> {
    if let Type::Path(path) = item.self_ty.as_ref() {
        return Some(path.path.get_ident()?.to_string());
    }

    None
}

fn to_camel_case(input: &str) -> String {
    let mut output = String::new();
    input.split('_').for_each(|item| {
        if output.is_empty() {
            output.push_str(item);
        } else {
            for (i, char) in item.chars().enumerate() {
                output.push(if i == 0 {
                    char.to_uppercase().to_string().chars().next().unwrap()
                } else {
                    char
                });
            }
        }
    });

    output
}

fn get_args_ident<T: IntoIterator<Item = FnArg> + Clone>(
    args: &T,
) -> Vec<proc_macro2::TokenStream> {
    let mut idents = Vec::new();
    for item in args.clone().into_iter() {
        if let FnArg::Typed(typed) = item {
            if let Pat::Ident(ident) = typed.pat.as_ref() {
                let name = ident.ident.clone();
                idents.push(quote! { #name, })
            }
        }
    }

    idents
}
