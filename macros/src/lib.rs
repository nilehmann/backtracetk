use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Data, Error, Ident};
use synstructure::Structure;

synstructure::decl_derive!([Partial, attributes(complete)] => partial_derive);

synstructure::decl_derive!([Complete] => complete_derive);

synstructure::decl_derive!([Partialize] => partialize_derive);

fn partial_derive(s: Structure) -> TokenStream {
    partial_derive_inner(s).unwrap_or_else(|err| err.to_compile_error())
}

fn partial_derive_inner(s: Structure) -> syn::Result<TokenStream> {
    let data = check_is_struct("Partial", &s)?;

    let complete_ident = find_complete_attr(&s.ast().attrs)?.parse_args::<Ident>()?;

    let merge_with_body: TokenStream = iter_fields(data)
        .map(|(f, _)| quote! { #f: other.#f.merge_with(self.#f), })
        .collect();

    let into_complete_body: TokenStream = iter_fields(data)
        .map(|(f, _)| quote! { #f: self.#f.into_complete(), })
        .collect();

    Ok(s.gen_impl(quote! {
        gen impl crate::partial::Partial for @Self {
            type Complete = #complete_ident;

            fn merge_with(mut self, other: Self) -> Self {
                Self { #merge_with_body }
            }

            fn into_complete(self) -> Self::Complete {
                #complete_ident { #into_complete_body }
            }
        }
    }))
}

fn find_complete_attr(attrs: &[syn::Attribute]) -> syn::Result<&syn::Attribute> {
    attrs
        .iter()
        .find(|attr| attr.path().is_ident("complete"))
        .ok_or_else(|| Error::new(Span::call_site(), "missing complete attr"))
}

fn complete_derive(s: Structure) -> TokenStream {
    s.gen_impl(quote! {
        gen impl crate::partial::Complete for @Self {
            type Partial = Option<Self>;
        }
    })
}

fn partialize_derive(s: Structure) -> TokenStream {
    partialize_derive_inner(s).unwrap_or_else(|err| err.to_compile_error())
}

fn partialize_derive_inner(s: Structure) -> syn::Result<TokenStream> {
    let data = check_is_struct("Partialize", &s)?;

    let ident = &s.ast().ident;
    let partial_ident = Ident::new(&format!("Partial{ident}"), Span::call_site());
    let partial_body: TokenStream = iter_fields(data)
        .map(|(f, ty)| quote! { #f: <#ty as crate::partial::Complete>::Partial, })
        .collect();

    let complete_impl = s.gen_impl(quote! {
        gen impl crate::partial::Complete for @Self {
            type Partial = #partial_ident;
        }
    });

    Ok(quote! {
        #[derive(Default, Deserialize, macros::Partial)]
        #[complete(#ident)]
        #[serde(default)]
        pub struct #partial_ident {
            #partial_body
        }

        #complete_impl
    })
}

fn iter_fields(data: &syn::DataStruct) -> impl Iterator<Item = (Ident, &syn::Type)> {
    data.fields
        .iter()
        .enumerate()
        .map(|(i, fld)| match fld.ident.as_ref() {
            Some(ident) => (ident.clone(), &fld.ty),
            None => (Ident::new(&format!("{i}"), Span::call_site()), &fld.ty),
        })
}

fn check_is_struct<'a>(trait_: &str, s: &'a Structure) -> syn::Result<&'a syn::DataStruct> {
    match &s.ast().data {
        Data::Struct(data) => Ok(data),
        _ => Err(Error::new_spanned(
            s.ast(),
            format!("{trait_} can only be derived for structs"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::merge_derive;

    #[test]
    fn test00() {
        synstructure::test_derive! {
            merge_derive {
                #[complete(A)]
                struct A {
                    a: i32,
                    b: i32,
                }
            }
            expands to {
                const _: () = {
                    impl crate::merge::Merge for A {
                        fn merge_with(mut self, other: Self) -> Self {
                            Self {
                                a: other.a.merge_with(self.a),
                                b: other.b.merge_with(self.b),
                            }
                        }

                        fn to_complete(mut self, other: Self) -> Self {
                            Self {
                                a: self.a.to_complete(),
                                b: self.b.to_complete(),
                            }
                        }
                    }
                };
            }
            no_build
        }
    }
}
