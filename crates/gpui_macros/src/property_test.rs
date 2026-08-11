use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote, quote_spanned};
use syn::{
    Attribute, Expr, FnArg, Ident, ItemFn, Meta, MetaNameValue, Pat, ReturnType, Token, Type,
    parse::{Parse, ParseStream},
    parse2,
    punctuated::Punctuated,
    spanned::Spanned,
    token::Comma,
};

pub fn test(args: TokenStream, item: TokenStream) -> TokenStream {
    let item_span = item.span();
    let Ok(func) = parse2::<ItemFn>(item) else {
        return quote_spanned! { item_span =>
            compile_error!("#[gpui::property_test] must be placed on a function");
        };
    };

    let args = match parse2::<Args>(args) {
        Ok(args) => args,
        Err(e) => return e.to_compile_error(),
    };

    let test_name = func.sig.ident.clone();
    let test_ret_ty = func.sig.output.clone();
    let inner_fn_name = format_ident!("__{test_name}");
    let outer_fn_attributes = &func.attrs;

    let parsed_args = parse_args(func.sig.inputs, &test_name);

    let inner_body = func.block;
    let inner_arg_decls = parsed_args.inner_fn_decl_args;
    let asyncness = func.sig.asyncness;

    let inner_fn = quote! {
        let #inner_fn_name = #asyncness move |#inner_arg_decls| #inner_body;
    };

    let inner_args = parsed_args.inner_fn_args;
    let cx_vars = parsed_args.cx_vars;
    let cx_teardowns = parsed_args.cx_teardowns;
    let property_test = render_property_test(parsed_args.property_args);
    let property_args_type = property_test.args_type;
    let property_args_definition = property_test.args_definition;
    let property_strategy = property_test.strategy;
    let property_args_pattern = property_test.args_pattern;
    let scheduler_seed = property_test.scheduler_seed;

    let run_test_body = match &asyncness {
        None => quote! {
            #cx_vars
            let result = #inner_fn_name(#inner_args);
            #cx_teardowns
            result
        },
        Some(_) => quote! {
            let foreground_executor = gpui::ForegroundExecutor::new(std::sync::Arc::new(dispatcher.clone()));
            #cx_vars
            let result = foreground_executor.block_test(#inner_fn_name(#inner_args));
            #cx_teardowns
            result
        },
    };

    let handle_result = match &test_ret_ty {
        ReturnType::Default => quote! {
            let (): () = result;
            ::core::result::Result::Ok(())
        },
        ReturnType::Type(_, ty) if matches!(ty.as_ref(), Type::Tuple(tuple) if tuple.elems.is_empty()) =>
        {
            quote! {
                let (): () = result;
                ::core::result::Result::Ok(())
            }
        }
        ReturnType::Type(_, _) => quote! { result },
    };

    let config = args.render_config();
    let argument_errors = parsed_args.errors;
    let option_errors = args.errors;

    quote! {
        #argument_errors
        #option_errors
        #(#outer_fn_attributes)*
        #[test]
        fn #test_name() {
            #property_args_definition

            let config = ::gpui::proptest::test_runner::Config {
                test_name: ::core::option::Option::Some(concat!(
                    module_path!(),
                    "::",
                    stringify!(#test_name),
                )),
                source_file: ::core::option::Option::Some(file!()),
                ..#config
            };
            let mut runner = ::gpui::proptest::test_runner::TestRunner::new(config);
            let strategy = #property_strategy;
            let result = runner.run(
                &strategy,
                |#property_args_type #property_args_pattern| {
                    #inner_fn

                    let result = ::gpui::run_test_once(
                        #scheduler_seed,
                        Box::new(move |dispatcher| #test_ret_ty {
                            #run_test_body
                        }),
                    );
                    #handle_result
                },
            );

            if let ::core::result::Result::Err(error) = result {
                panic!("{}\n{}", error, runner);
            }
        }
    }
}

struct Args {
    config: Option<Expr>,
    errors: TokenStream,
}

impl Args {
    /// By default, proptest uses random seeds unless `$PROPTEST_SEED` is set.
    /// Rather than managing both `$SEED` and `$PROPTEST_SEED`, we intercept
    /// `config = ...` tokens and add a call to `gpui::apply_seed_to_config`.
    fn render_config(&self) -> TokenStream {
        let user_provided_config = match &self.config {
            None => quote! { ::gpui::proptest::prelude::ProptestConfig::default() },
            Some(config) => config.into_token_stream(),
        };

        quote!(::gpui::apply_seed_to_proptest_config(#user_provided_config))
    }
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let pairs = Punctuated::<MetaNameValue, Token![,]>::parse_terminated(input)?;

        let mut config = None;
        let mut errors = quote!();

        for pair in pairs {
            match pair.path.get_ident().map(Ident::to_string).as_deref() {
                Some("config") => config = Some(pair.value),
                Some("proptest_path") => errors.extend(quote_spanned! {pair.span() =>
                    compile_error!("`gpui::property_test` always uses GPUI's proptest re-export");
                }),
                Some(other) => {
                    let message = format!("unknown `gpui::property_test` argument: {other}");
                    errors.extend(quote_spanned! {pair.span() =>
                        compile_error!(#message);
                    });
                }
                None => errors.extend(quote_spanned! {pair.span() =>
                    compile_error!("unknown `gpui::property_test` argument");
                }),
            }
        }

        Ok(Self { config, errors })
    }
}

#[derive(Default)]
struct ParsedArgs {
    cx_vars: TokenStream,
    cx_teardowns: TokenStream,
    errors: TokenStream,
    property_args: Vec<PropertyArg>,

    // exprs passed at the call-site
    inner_fn_args: TokenStream,
    // args in the declaration
    inner_fn_decl_args: TokenStream,
}

fn parse_args(args: Punctuated<FnArg, Comma>, test_name: &Ident) -> ParsedArgs {
    let mut parsed = ParsedArgs::default();
    let mut args = args.into_iter().collect();

    remove_cxs(&mut parsed, &mut args, test_name);
    remove_std_rng(&mut parsed, &mut args);
    remove_background_executor(&mut parsed, &mut args);
    parse_property_args(&mut parsed, args);

    parsed
}

struct PropertyArg {
    field: Ident,
    value: Ident,
    pattern: Box<Pat>,
    ty: Box<Type>,
    strategy: TokenStream,
}

struct RenderedPropertyTest {
    args_type: Ident,
    args_definition: TokenStream,
    strategy: TokenStream,
    args_pattern: TokenStream,
    scheduler_seed: Ident,
}

fn parse_property_args(parsed: &mut ParsedArgs, args: Vec<FnArg>) {
    for (index, arg) in args.into_iter().enumerate() {
        let FnArg::Typed(mut arg) = arg else {
            parsed.errors.extend(quote_spanned! {arg.span() =>
                compile_error!("`self` parameters are forbidden in property tests");
            });
            continue;
        };

        let strategy = take_strategy(&mut parsed.errors, &arg.attrs, &arg.pat)
            .map(|strategy| strategy.into_token_stream())
            .unwrap_or_else(|| {
                let ty = &arg.ty;
                quote!(::gpui::proptest::prelude::any::<#ty>())
            });
        arg.attrs.clear();

        parsed.property_args.push(PropertyArg {
            field: format_ident!("__gpui_property_field_{index}"),
            value: format_ident!("__gpui_property_value_{index}"),
            pattern: arg.pat,
            ty: arg.ty,
            strategy,
        });
    }
}

fn take_strategy(errors: &mut TokenStream, attrs: &[Attribute], pattern: &Pat) -> Option<Expr> {
    let mut strategy = None;

    for attr in attrs {
        if !attr.path().is_ident("strategy") {
            errors.extend(quote_spanned! {attr.span() =>
                compile_error!("only `#[strategy = <expr>]` attributes are allowed on property-test values");
            });
            continue;
        }

        let Meta::NameValue(name_value) = &attr.meta else {
            errors.extend(quote_spanned! {attr.meta.span() =>
                compile_error!("`strategy` attributes must have the form `#[strategy = <expr>]`");
            });
            continue;
        };

        if strategy.is_some() {
            let message = format!(
                "{} has more than one `#[strategy = ...]` attribute",
                pattern.to_token_stream(),
            );
            errors.extend(quote_spanned! {attr.span() =>
                compile_error!(#message);
            });
        } else {
            strategy = Some(name_value.value.clone());
        }
    }

    strategy
}

fn render_property_test(args: Vec<PropertyArg>) -> RenderedPropertyTest {
    let args_type = format_ident!("__GpuiPropertyTestArgs");
    let scheduler_seed = format_ident!("__gpui_property_scheduler_seed");
    let seed_field = format_ident!("__gpui_property_seed");
    let seed_value = format_ident!("__gpui_property_seed_value");

    let fields = args.iter().map(|arg| {
        let field = &arg.field;
        let ty = &arg.ty;
        quote!(#field: #ty,)
    });
    let args_definition = quote! {
        #[derive(Debug)]
        struct #args_type {
            #seed_field: u64,
            #(#fields)*
        }
    };

    let mut combined_strategy = quote!(::gpui::seed_strategy());
    let mut combined_values = quote!(#seed_value);
    for arg in &args {
        let strategy = &arg.strategy;
        let value = &arg.value;
        combined_strategy = quote!((#combined_strategy, #strategy));
        combined_values = quote!((#combined_values, #value));
    }

    let field_values = args.iter().map(|arg| {
        let field = &arg.field;
        let value = &arg.value;
        quote!(#field: #value,)
    });
    let strategy = quote! {
        ::gpui::proptest::strategy::Strategy::prop_map(
            #combined_strategy,
            |#combined_values| #args_type {
                #seed_field: #seed_value,
                #(#field_values)*
            },
        )
    };

    let field_patterns = args.iter().map(|arg| {
        let field = &arg.field;
        let pattern = &arg.pattern;
        quote!(#field: #pattern,)
    });
    let args_pattern = quote! {
        {
            #seed_field: #scheduler_seed,
            #(#field_patterns)*
        }
    };

    RenderedPropertyTest {
        args_type,
        args_definition,
        strategy,
        args_pattern,
        scheduler_seed,
    }
}

fn remove_cxs(parsed: &mut ParsedArgs, args: &mut Vec<FnArg>, test_name: &Ident) {
    let mut ix = 0;
    args.retain_mut(|arg| {
        if !is_test_cx(arg) {
            return true;
        }

        let cx_varname = format_ident!("cx_{ix}");
        ix += 1;

        parsed.cx_vars.extend(quote!(
            let mut #cx_varname = gpui::TestAppContext::build(
                dispatcher.clone(),
                Some(stringify!(#test_name)),
            );
        ));
        parsed.cx_teardowns.extend(quote!(
            dispatcher.run_until_parked();
            #cx_varname.executor().forbid_parking();
            #cx_varname.quit();
            dispatcher.run_until_parked();
        ));

        parsed.inner_fn_decl_args.extend(quote!(#arg,));
        parsed.inner_fn_args.extend(quote!(&mut #cx_varname,));

        false
    });
}

fn remove_std_rng(parsed: &mut ParsedArgs, args: &mut Vec<FnArg>) {
    args.retain_mut(|arg| {
        if !is_std_rng(arg) {
            return true;
        }

        parsed.errors.extend(quote_spanned! { arg.span() =>
            compile_error!("`StdRng` is not allowed in a property test. Consider implementing `Arbitrary`, or implementing a custom `Strategy`. https://altsysrq.github.io/proptest-book/proptest/tutorial/strategy-basics.html");
        });

        false
    });
}

fn remove_background_executor(parsed: &mut ParsedArgs, args: &mut Vec<FnArg>) {
    args.retain_mut(|arg| {
        if !is_background_executor(arg) {
            return true;
        }

        parsed.inner_fn_decl_args.extend(quote!(#arg,));
        parsed
            .inner_fn_args
            .extend(quote!(gpui::BackgroundExecutor::new(std::sync::Arc::new(
                dispatcher.clone()
            )),));

        false
    });
}

// Matches `&TestAppContext` or `&foo::bar::baz::TestAppContext`
fn is_test_cx(arg: &FnArg) -> bool {
    let FnArg::Typed(arg) = arg else {
        return false;
    };

    let Type::Reference(ty) = &*arg.ty else {
        return false;
    };

    let Type::Path(ty) = &*ty.elem else {
        return false;
    };

    ty.path
        .segments
        .last()
        .is_some_and(|seg| seg.ident == "TestAppContext")
}

fn is_std_rng(arg: &FnArg) -> bool {
    is_path_with_last_segment(arg, "StdRng")
}

fn is_background_executor(arg: &FnArg) -> bool {
    is_path_with_last_segment(arg, "BackgroundExecutor")
}

fn is_path_with_last_segment(arg: &FnArg, last_segment: &str) -> bool {
    let FnArg::Typed(arg) = arg else {
        return false;
    };

    let Type::Path(ty) = &*arg.ty else {
        return false;
    };

    ty.path
        .segments
        .last()
        .is_some_and(|seg| seg.ident == last_segment)
}
