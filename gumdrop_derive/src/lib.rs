//! Provides `derive(Options)` for `gumdrop` crate
//!
//! # `derive(Options)`
//!
//! `derive(Options)` generates an implementation of the trait `Options`,
//! creating an option for each field of the decorated `struct`.
//!
//! See the `gumdrop` [documentation](https://docs.rs/gumdrop/) for an example
//! of its usage.
//!
//! ## `options` attribute
//!
//! Behavior of `derive(Options)` can be controlled by adding `#[options(...)]`
//! attributes to one or more fields within a decorated struct.
//!
//! Supported items are:
//!
//! * `command` indicates that a field represents a subcommand. The field must
//!   be of type `Option<T>` where `T` is a type implementing `Options`.
//!   Typically, this type is an `enum` containing subcommand option types.
//! * `help_flag` marks an option as a help flag. The field must be `bool` type.
//!   Options named `help` will automatically receive this option.
//! * `no_help_flag` prevents an option from being considered a help flag.
//! * `count` marks a field as a counter value. The field will be incremented
//!   each time the option appears in the arguments, i.e. `field += 1;`
//! * `free` marks a field as a positional argument field. Non-option arguments
//!   will be used to fill all `free` fields, in declared sequence.
//!   If the final `free` field is of type `Vec<T>`, it will contain all
//!   remaining free arguments.
//! * `short = "?"` sets the short option name to the given character
//! * `no_short` prevents a short option from being assigned to the field
//! * `long = "..."` sets the long option name to the given string
//! * `no_long` prevents a long option from being assigned to the field
//! * `default` provides a default value for the option field.
//!   The value of this field is parsed in the same way as argument values.
//! * `default_expr` provides a default value for the option field.
//!   The value of this field is parsed at compile time as a Rust expression
//!   and is evaluated before any argument values are processed.  
//!   The `default_expr` feature must be enabled to use this attribute.
//! * `required` will cause an error if the option is not present,
//!   unless at least one `help_flag` option is also present.
//! * `multi = "..."` will allow parsing an option multiple times,
//!   adding each parsed value to the field using the named method.
//!   This behavior is automatically applied to `Vec<T>` fields, unless the
//!   `no_multi` option is present.
//! * `no_multi` will inhibit automatically marking `Vec<T>` fields as `multi`
//! * `not_required` will cancel a type-level `required` flag (see below).
//! * `help = "..."` sets help text returned from the `Options::usage` method;
//!   field doc comment may also be provided to set the help text.
//!   If both are present, the `help` attribute value is used.
//! * `meta = "..."` sets the meta variable displayed in usage for options
//!   which accept an argument
//! * `parse(...)` uses a named function to parse a value from a string.
//!   Valid parsing function types are:
//!     * `parse(from_str = "...")` for `fn(&str) -> T`
//!     * `parse(try_from_str = "...")` for
//!       `fn(&str) -> Result<T, E> where E: Display`
//!     * `parse(from_str)` uses `std::convert::From::from`
//!     * `parse(try_from_str)` uses `std::str::FromStr::from_str`
//!
//! The `options` attribute may also be added at the type level.
//!
//! The `help` attribute (or a type-level doc comment) can be used to provide
//! some introductory text which will precede option help text in the usage
//! string.
//!
//! Additionally, the following flags may be set at the type level to establish
//! default values for all contained fields: `no_help_flag`, `no_long`,
//! `no_short`, and `required`.

#![recursion_limit = "1024"]

extern crate proc_macro;

use std::iter::repeat;

use quote::quote;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};

use syn::{
    parse::Error, spanned::Spanned,
    Attribute, AttrStyle, Data, DataEnum, DataStruct, DeriveInput, Fields,
    GenericArgument, Ident, Lit, Meta, NestedMeta, Path, PathArguments, Type,
    parse_str,
};

#[cfg(feature = "default_expr")]
use syn::Expr;

#[proc_macro_derive(OptionsCore, attributes(options))]
pub fn derive_options_core(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = match syn::parse(input) {
        Ok(ast) => ast,
        Err(e) => {
            return e.to_compile_error().into();
        }
    };

    let span = ast.ident.span();

    let result = match &ast.data {
        Data::Enum(data) =>
            derive_optionscore_enum(&ast, data),
        Data::Struct(DataStruct{fields: Fields::Unit, ..}) =>
            Err(Error::new(span, "cannot derive Options for unit struct types")),
        Data::Struct(DataStruct{fields: Fields::Unnamed(..), ..}) =>
            Err(Error::new(span, "cannot derive Options for tuple struct types")),
        Data::Struct(DataStruct{fields, ..}) =>
            derive_optionscore_struct(&ast, fields),
        Data::Union(_) =>
            Err(Error::new(span, "cannot derive Options for union types")),
    };

    match result {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into()
    }
}

fn derive_optionscore_enum(ast: &DeriveInput, data: &DataEnum)
        -> Result<TokenStream2, Error> {
    let name = &ast.ident;
    let mut commands = Vec::new();
    let mut var_ty = Vec::new();

    for var in &data.variants {
        let span = var.ident.span();

        let ty = match &var.fields {
            Fields::Unit | Fields::Named(_) =>
                return Err(Error::new(span,
                    "command variants must be unary tuple variants")),
            Fields::Unnamed(fields) if fields.unnamed.len() != 1 =>
                return Err(Error::new(span,
                    "command variants must be unary tuple variants")),
            Fields::Unnamed(fields) =>
                &fields.unnamed.first().unwrap().ty,
        };

        let opts = CmdOpts::parse(&var.attrs)?;

        let var_name = &var.ident;

        var_ty.push(ty);

        commands.push(Cmd{
            name: opts.name.unwrap_or_else(
                || make_command_name(&var_name.to_string())),
            help: opts.help.or(opts.doc),
            variant_name: var_name,
            ty: ty,
        });
    }

    let mut command = Vec::new();
    let mut handle_cmd = Vec::new();
    let mut help_req_impl = Vec::new();
    let mut variant = Vec::new();

    for cmd in commands {
        command.push(cmd.name);

        let var_name = cmd.variant_name;
        let ty = &cmd.ty;

        variant.push(var_name);

        handle_cmd.push(quote!{
            #name::#var_name(<#ty as ::gumdrop::OptionsCore>::parse(_parser)?)
        });

        help_req_impl.push(quote!{
            #name::#var_name(cmd) => { ::gumdrop::OptionsCore::help_requested(cmd) }
        });
    }

    // Borrow re-used items
    let command = &command;

    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    Ok(quote!{
        impl #impl_generics ::gumdrop::OptionsCore for #name #ty_generics #where_clause {
            fn parse<__S: ::std::convert::AsRef<str>>(
                    _parser: &mut ::gumdrop::Parser<__S>)
                    -> ::std::result::Result<Self, ::gumdrop::Error> {
                let _arg = _parser.next_arg()
                    .ok_or_else(::gumdrop::Error::missing_command)?;

                Self::parse_command(_arg, _parser)
            }

            fn parse_command<__S: ::std::convert::AsRef<str>>(name: &str,
                    _parser: &mut ::gumdrop::Parser<__S>)
                    -> ::std::result::Result<Self, ::gumdrop::Error> {
                let cmd = match name {
                    #( #command => { #handle_cmd } )*
                    _ => return ::std::result::Result::Err(
                        ::gumdrop::Error::unrecognized_command(name))
                };

                ::std::result::Result::Ok(cmd)
            }
        }
    })
}

fn derive_optionscore_struct(
    ast: &DeriveInput,
    fields: &Fields
) -> Result<TokenStream2, Error> {
    let mut pattern = Vec::new();
    let mut handle_opt = Vec::new();
    let mut short_names = Vec::new();
    let mut long_names = Vec::new();
    let mut free: Vec<FreeOpt> = Vec::new();
    let mut required = Vec::new();
    let mut required_err = Vec::new();
    let mut command = None;
    let mut command_required = false;
    let mut help_flag = Vec::new();
    let mut options = Vec::new();
    let mut field_name = Vec::new();
    let mut default = Vec::new();

    let default_expr = quote!{ ::std::default::Default::default() };
    let default_opts = DefaultOpts::parse(&ast.attrs)?;

    for field in fields {
        let span = field.ident.as_ref().unwrap().span();

        let mut opts = AttrOpts::parse(span, &field.attrs)?;
        opts.set_defaults(&default_opts);

        let ident = field.ident.as_ref().unwrap();

        field_name.push(ident);

        if let Some(expr) = &opts.default {
            default.push(opts.parse.as_ref()
                .unwrap_or(&ParseFn::Default)
                .make_parse_default_action(ident, &expr));
        } else {
            #[cfg(not(feature = "default_expr"))]
            default.push(default_expr.clone());

            #[cfg(feature = "default_expr")]
            {
                if let Some(expr) = &opts.default_expr {
                    default.push(quote!{ #expr });
                } else {
                    default.push(default_expr.clone());
                }
            }
        }

        if opts.command {
            if command.is_some() {
                return Err(Error::new(span,
                    "duplicate declaration of `command` field"));
            }
            if !free.is_empty() {
                return Err(Error::new(span,
                    "`command` and `free` options are mutually exclusive"));
            }

            command = Some(ident);
            command_required = opts.required;

            if opts.required {
                required.push(ident);
                required_err.push(quote!{
                    ::gumdrop::Error::missing_required_command() });
            }

            continue;
        }

        if opts.free {
            if command.is_some() {
                return Err(Error::new(span,
                    "`command` and `free` options are mutually exclusive"));
            }

            if let Some(last) = free.last() {
                if last.action.is_push() {
                    return Err(Error::new(span,
                        "only the final `free` option may be of type `Vec<T>`"));
                }
            }

            if opts.required {
                required.push(ident);
                required_err.push(quote!{
                    ::gumdrop::Error::missing_required_free() });
            }

            free.push(FreeOpt{
                field: ident,
                action: FreeAction::infer(&field.ty, &opts),
                parse: opts.parse.unwrap_or_default(),
                required: opts.required,
                help: opts.help.or(opts.doc),
            });

            continue;
        }

        if opts.long.is_none() && !opts.no_long {
            opts.long = Some(make_long_name(&ident.to_string()));
        }

        if let Some(long) = &opts.long {
            validate_long_name(span, long, &long_names)?;
            long_names.push(long.clone());
        }

        if let Some(short) = opts.short {
            validate_short_name(span, short, &short_names)?;
            short_names.push(short);
        }

        if opts.help_flag || (!opts.no_help_flag &&
                opts.long.as_ref().map(|s| &s[..]) == Some("help")) {
            help_flag.push(ident);
        }

        let action = if opts.count {
            Action::Count
        } else {
            Action::infer(&field.ty, &opts)
        };

        if action.takes_arg() {
            if opts.meta.is_none() {
                opts.meta = Some(make_meta(&ident.to_string(), &action));
            }
        } else if opts.meta.is_some() {
            return Err(Error::new(span,
                "`meta` value is invalid for this field"));
        }

        options.push(Opt{
            field: ident,
            action: action,
            long: opts.long,
            short: opts.short,
            no_short: opts.no_short,
            required: opts.required,
            meta: opts.meta,
            help: opts.help.or(opts.doc),
            default: opts.default,
        });
    }

    // do not make short automatically
    // only if user explicitly requested short options

    for opt in &options {
        if opt.required {
            required.push(opt.field);
            let display = opt.display_form();
            required_err.push(quote!{
                ::gumdrop::Error::missing_required(#display) });
        }

        let pat = match (&opt.long, opt.short) {
            (Some(long), Some(short)) => quote!{
                ::gumdrop::Opt::Long(#long) | ::gumdrop::Opt::Short(#short)
            },
            (Some(long), None) => quote!{
                ::gumdrop::Opt::Long(#long)
            },
            (None, Some(short)) => quote!{
                ::gumdrop::Opt::Short(#short)
            },
            (None, None) => {
                return Err(Error::new(opt.field.span(),
                    "option has no long or short flags"));
            }
        };

        pattern.push(pat);
        handle_opt.push(opt.make_action());

        if let Some(long) = &opt.long {
            let (pat, handle) = if let Some(n) = opt.action.tuple_len() {
                (quote!{ ::gumdrop::Opt::LongWithArg(#long, _) },
                    quote!{ return ::std::result::Result::Err(
                        ::gumdrop::Error::unexpected_single_argument(_opt, #n)) })
            } else if opt.action.takes_arg() {
                (quote!{ ::gumdrop::Opt::LongWithArg(#long, _arg) },
                    opt.make_action_arg())
            } else {
                (quote!{ ::gumdrop::Opt::LongWithArg(#long, _) },
                    quote!{ return ::std::result::Result::Err(
                        ::gumdrop::Error::unexpected_argument(_opt)) })
            };

            pattern.push(pat);
            handle_opt.push(handle);
        }
    }

    let name = &ast.ident;

    let handle_free = if !free.is_empty() {
        let catch_all = if free.last().unwrap().action.is_push() {
            let last = free.pop().unwrap();

            let free = last.field;
            let name = free.to_string();
            let meth = match &last.action {
                FreeAction::Push(meth) => meth,
                _ => unreachable!()
            };

            let parse = last.parse.make_parse_action(Some(&name[..]));
            let mark_used = last.mark_used();

            quote!{
                #mark_used
                let _arg = _free;
                _result.#free.#meth(#parse);
            }
        } else {
            quote!{
                // ignore unrecognized frees
                // return ::std::result::Result::Err(
                //     ::gumdrop::Error::unexpected_free(_free))
            }
        };

        let num = 0..free.len();
        let action = free.iter().map(|free| {
            let field = free.field;
            let name = field.to_string();

            let mark_used = free.mark_used();
            let parse = free.parse.make_parse_action(Some(&name[..]));

            let assign = match &free.action {
                FreeAction::Push(meth) => quote!{
                    let _arg = _free;
                    _result.#field.#meth(#parse);
                },
                FreeAction::SetField => quote!{
                    let _arg = _free;
                    _result.#field = #parse;
                },
                FreeAction::SetOption => quote!{
                    let _arg = _free;
                    _result.#field = ::std::option::Option::Some(#parse);
                },
            };

            quote!{
                #mark_used
                #assign
            }
        }).collect::<Vec<_>>();

        quote!{
            match _free_counter {
                #( #num => {
                    _free_counter += 1;
                    #action
                } )*
                _ => { #catch_all }
            }
        }
    } else if let Some(ident) = command {
        let mark_used = if command_required {
            quote!{
                _used.#ident = true;
            }
        } else {
            quote!{
            }
        };

        quote!{
            #mark_used
            _result.#ident = ::std::option::Option::Some(
                ::gumdrop::OptionsCore::parse_command(_free, _parser)?);
            break;
        }
    } else {
        quote!{
            // I dont think we should error on an
            // unexpected free positional
            // return ::std::result::Result::Err(
            //     ::gumdrop::Error::unexpected_free(_free));
        }
    };

    let required = &required;

    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    Ok(quote!{
        impl #impl_generics ::gumdrop::OptionsCore for #name #ty_generics #where_clause {
            fn parse<__S: ::std::convert::AsRef<str>>(
                    _parser: &mut ::gumdrop::Parser<__S>)
                    -> ::std::result::Result<Self, ::gumdrop::Error> {
                #[derive(Default)]
                struct _Used {
                    #( #required: bool , )*
                }

                let mut _result = #name{
                    #( #field_name: #default ),*
                };
                let mut _free_counter = 0usize;
                let mut _used = _Used::default();

                while let ::std::option::Option::Some(_opt) = _parser.next_opt() {
                    match _opt {
                        #( #pattern => {
                            #handle_opt
                        } )*
                        ::gumdrop::Opt::Free(_free) => {
                            #handle_free
                        }
                        _ => {
                            // I dont think its a good idea to error if
                            // we found unrecognized input. maybe give a warning?
                            // return ::std::result::Result::Err(
                            //     ::gumdrop::Error::unrecognized_option(_opt));
                        }
                    }
                }

                if true #( && !_result.#help_flag )* {
                    #( if !_used.#required {
                        return ::std::result::Result::Err(#required_err);
                    } )*
                }

                ::std::result::Result::Ok(_result)
            }

            fn parse_command<__S: ::std::convert::AsRef<str>>(
                name: &str,
                _parser: &mut ::gumdrop::Parser<__S>
            ) -> ::std::result::Result<Self, ::gumdrop::Error> {
                ::std::result::Result::Err(
                    ::gumdrop::Error::unrecognized_command(name)
                )
            }
        }
    })
}


/// Derives the `gumdrop::Options` trait for `struct` and `enum` items.
///
/// `#[options(...)]` attributes can be used to control behavior of generated trait
/// implementation. See [crate-level documentation](index.html) for the full list of
/// supported options.
#[proc_macro_derive(Options, attributes(options))]
pub fn derive_options(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = match syn::parse(input) {
        Ok(ast) => ast,
        Err(e) => {
            return e.to_compile_error().into();
        }
    };

    let span = ast.ident.span();

    let result = match &ast.data {
        Data::Enum(data) =>
            derive_options_enum(&ast, data),
        Data::Struct(DataStruct{fields: Fields::Unit, ..}) =>
            Err(Error::new(span, "cannot derive Options for unit struct types")),
        Data::Struct(DataStruct{fields: Fields::Unnamed(..), ..}) =>
            Err(Error::new(span, "cannot derive Options for tuple struct types")),
        Data::Struct(DataStruct{fields, ..}) =>
            derive_options_struct(&ast, fields),
        Data::Union(_) =>
            Err(Error::new(span, "cannot derive Options for union types")),
    };

    match result {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into()
    }
}

fn derive_options_enum(ast: &DeriveInput, data: &DataEnum)
        -> Result<TokenStream2, Error> {
    let name = &ast.ident;
    let mut commands = Vec::new();
    let mut var_ty = Vec::new();

    for var in &data.variants {
        let span = var.ident.span();

        let ty = match &var.fields {
            Fields::Unit | Fields::Named(_) =>
                return Err(Error::new(span,
                    "command variants must be unary tuple variants")),
            Fields::Unnamed(fields) if fields.unnamed.len() != 1 =>
                return Err(Error::new(span,
                    "command variants must be unary tuple variants")),
            Fields::Unnamed(fields) =>
                &fields.unnamed.first().unwrap().ty,
        };

        let opts = CmdOpts::parse(&var.attrs)?;

        let var_name = &var.ident;

        var_ty.push(ty);

        commands.push(Cmd{
            name: opts.name.unwrap_or_else(
                || make_command_name(&var_name.to_string())),
            help: opts.help.or(opts.doc),
            variant_name: var_name,
            ty: ty,
        });
    }

    let mut command = Vec::new();
    let mut handle_cmd = Vec::new();
    let mut help_req_impl = Vec::new();
    let mut variant = Vec::new();
    let usage = make_cmd_usage(&commands);

    for cmd in commands {
        command.push(cmd.name);

        let var_name = cmd.variant_name;
        let ty = &cmd.ty;

        variant.push(var_name);

        handle_cmd.push(quote!{
            #name::#var_name(<#ty as ::gumdrop::Options>::parse(_parser)?)
        });

        help_req_impl.push(quote!{
            #name::#var_name(cmd) => { ::gumdrop::Options::help_requested(cmd) }
        });
    }

    // Borrow re-used items
    let command = &command;

    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    let command_impl = {
        let name = repeat(name);

        quote!{
            match self {
                #( #name::#variant(cmd) => ::gumdrop::Options::command(cmd), )*
            }
        }
    };

    let command_name_impl = {
        let name = repeat(name);

        quote!{
            match self {
                #( #name::#variant(_) => ::std::option::Option::Some(#command), )*
            }
        }
    };

    let self_usage_impl = {
        let name = repeat(name);

        quote!{
            match self {
                #( #name::#variant(sub) => ::gumdrop::Options::self_usage(sub), )*
            }
        }
    };

    let self_command_list_impl = {
        let name = repeat(name);

        quote!{
            match self {
                #( #name::#variant(sub) => ::gumdrop::Options::self_command_list(sub), )*
            }
        }
    };

    Ok(quote!{
        impl #impl_generics ::gumdrop::Options for #name #ty_generics #where_clause {
            fn parse<__S: ::std::convert::AsRef<str>>(
                    _parser: &mut ::gumdrop::Parser<__S>)
                    -> ::std::result::Result<Self, ::gumdrop::Error> {
                let _arg = _parser.next_arg()
                    .ok_or_else(::gumdrop::Error::missing_command)?;

                Self::parse_command(_arg, _parser)
            }

            fn command(&self) -> ::std::option::Option<&dyn ::gumdrop::Options> {
                #command_impl
            }

            fn command_name(&self) -> ::std::option::Option<&'static str> {
                #command_name_impl
            }

            fn help_requested(&self) -> bool {
                match self {
                    #( #help_req_impl )*
                }
            }

            fn parse_command<__S: ::std::convert::AsRef<str>>(name: &str,
                    _parser: &mut ::gumdrop::Parser<__S>)
                    -> ::std::result::Result<Self, ::gumdrop::Error> {
                let cmd = match name {
                    #( #command => { #handle_cmd } )*
                    _ => return ::std::result::Result::Err(
                        ::gumdrop::Error::unrecognized_command(name))
                };

                ::std::result::Result::Ok(cmd)
            }

            fn usage() -> &'static str {
                #usage
            }

            fn self_usage(&self) -> &'static str {
                #self_usage_impl
            }

            fn command_list() -> ::std::option::Option<&'static str> {
                ::std::option::Option::Some(<Self as ::gumdrop::Options>::usage())
            }

            fn self_command_list(&self) -> ::std::option::Option<&'static str> {
                #self_command_list_impl
            }

            fn command_usage(name: &str) -> ::std::option::Option<&'static str> {
                match name {
                    #( #command => ::std::option::Option::Some(
                        <#var_ty as ::gumdrop::Options>::usage()), )*
                    _ => ::std::option::Option::None
                }
            }
        }
    })
}

fn derive_options_struct(ast: &DeriveInput, fields: &Fields)
        -> Result<TokenStream2, Error> {
    let mut pattern = Vec::new();
    let mut handle_opt = Vec::new();
    let mut short_names = Vec::new();
    let mut long_names = Vec::new();
    let mut free: Vec<FreeOpt> = Vec::new();
    let mut required = Vec::new();
    let mut required_err = Vec::new();
    let mut command = None;
    let mut command_ty = None;
    let mut command_required = false;
    let mut help_flag = Vec::new();
    let mut options = Vec::new();
    let mut field_name = Vec::new();
    let mut default = Vec::new();

    let default_expr = quote!{ ::std::default::Default::default() };
    let default_opts = DefaultOpts::parse(&ast.attrs)?;

    for field in fields {
        let span = field.ident.as_ref().unwrap().span();

        let mut opts = AttrOpts::parse(span, &field.attrs)?;
        opts.set_defaults(&default_opts);

        let ident = field.ident.as_ref().unwrap();

        field_name.push(ident);

        if let Some(expr) = &opts.default {
            default.push(opts.parse.as_ref()
                .unwrap_or(&ParseFn::Default)
                .make_parse_default_action(ident, &expr));
        } else {
            #[cfg(not(feature = "default_expr"))]
            default.push(default_expr.clone());

            #[cfg(feature = "default_expr")]
            {
                if let Some(expr) = &opts.default_expr {
                    default.push(quote!{ #expr });
                } else {
                    default.push(default_expr.clone());
                }
            }
        }

        if opts.command {
            if command.is_some() {
                return Err(Error::new(span,
                    "duplicate declaration of `command` field"));
            }
            if !free.is_empty() {
                return Err(Error::new(span,
                    "`command` and `free` options are mutually exclusive"));
            }

            command = Some(ident);
            command_ty = Some(first_ty_param(&field.ty).unwrap_or(&field.ty));
            command_required = opts.required;

            if opts.required {
                required.push(ident);
                required_err.push(quote!{
                    ::gumdrop::Error::missing_required_command() });
            }

            continue;
        }

        if opts.free {
            if command.is_some() {
                return Err(Error::new(span,
                    "`command` and `free` options are mutually exclusive"));
            }

            if let Some(last) = free.last() {
                if last.action.is_push() {
                    return Err(Error::new(span,
                        "only the final `free` option may be of type `Vec<T>`"));
                }
            }

            if opts.required {
                required.push(ident);
                required_err.push(quote!{
                    ::gumdrop::Error::missing_required_free() });
            }

            free.push(FreeOpt{
                field: ident,
                action: FreeAction::infer(&field.ty, &opts),
                parse: opts.parse.unwrap_or_default(),
                required: opts.required,
                help: opts.help.or(opts.doc),
            });

            continue;
        }

        if opts.long.is_none() && !opts.no_long {
            opts.long = Some(make_long_name(&ident.to_string()));
        }

        if let Some(long) = &opts.long {
            validate_long_name(span, long, &long_names)?;
            long_names.push(long.clone());
        }

        if let Some(short) = opts.short {
            validate_short_name(span, short, &short_names)?;
            short_names.push(short);
        }

        if opts.help_flag || (!opts.no_help_flag &&
                opts.long.as_ref().map(|s| &s[..]) == Some("help")) {
            help_flag.push(ident);
        }

        let action = if opts.count {
            Action::Count
        } else {
            Action::infer(&field.ty, &opts)
        };

        if action.takes_arg() {
            if opts.meta.is_none() {
                opts.meta = Some(make_meta(&ident.to_string(), &action));
            }
        } else if opts.meta.is_some() {
            return Err(Error::new(span,
                "`meta` value is invalid for this field"));
        }

        options.push(Opt{
            field: ident,
            action: action,
            long: opts.long,
            short: opts.short,
            no_short: opts.no_short,
            required: opts.required,
            meta: opts.meta,
            help: opts.help.or(opts.doc),
            default: opts.default,
        });
    }

    // Assign short names after checking all options.
    // Thus, manual short names will take priority over automatic ones.
    for opt in &mut options {
        if opt.short.is_none() && !opt.no_short {
            let short = make_short_name(&opt.field.to_string(), &short_names);

            if let Some(short) = short {
                short_names.push(short);
            }

            opt.short = short;
        }
    }

    for opt in &options {
        if opt.required {
            required.push(opt.field);
            let display = opt.display_form();
            required_err.push(quote!{
                ::gumdrop::Error::missing_required(#display) });
        }

        let pat = match (&opt.long, opt.short) {
            (Some(long), Some(short)) => quote!{
                ::gumdrop::Opt::Long(#long) | ::gumdrop::Opt::Short(#short)
            },
            (Some(long), None) => quote!{
                ::gumdrop::Opt::Long(#long)
            },
            (None, Some(short)) => quote!{
                ::gumdrop::Opt::Short(#short)
            },
            (None, None) => {
                return Err(Error::new(opt.field.span(),
                    "option has no long or short flags"));
            }
        };

        pattern.push(pat);
        handle_opt.push(opt.make_action());

        if let Some(long) = &opt.long {
            let (pat, handle) = if let Some(n) = opt.action.tuple_len() {
                (quote!{ ::gumdrop::Opt::LongWithArg(#long, _) },
                    quote!{ return ::std::result::Result::Err(
                        ::gumdrop::Error::unexpected_single_argument(_opt, #n)) })
            } else if opt.action.takes_arg() {
                (quote!{ ::gumdrop::Opt::LongWithArg(#long, _arg) },
                    opt.make_action_arg())
            } else {
                (quote!{ ::gumdrop::Opt::LongWithArg(#long, _) },
                    quote!{ return ::std::result::Result::Err(
                        ::gumdrop::Error::unexpected_argument(_opt)) })
            };

            pattern.push(pat);
            handle_opt.push(handle);
        }
    }

    let name = &ast.ident;
    let opts_help = default_opts.help.or(default_opts.doc);
    let usage = make_usage(&opts_help, &free, &options);

    let handle_free = if !free.is_empty() {
        let catch_all = if free.last().unwrap().action.is_push() {
            let last = free.pop().unwrap();

            let free = last.field;
            let name = free.to_string();
            let meth = match &last.action {
                FreeAction::Push(meth) => meth,
                _ => unreachable!()
            };

            let parse = last.parse.make_parse_action(Some(&name[..]));
            let mark_used = last.mark_used();

            quote!{
                #mark_used
                let _arg = _free;
                _result.#free.#meth(#parse);
            }
        } else {
            quote!{
                return ::std::result::Result::Err(
                    ::gumdrop::Error::unexpected_free(_free))
            }
        };

        let num = 0..free.len();
        let action = free.iter().map(|free| {
            let field = free.field;
            let name = field.to_string();

            let mark_used = free.mark_used();
            let parse = free.parse.make_parse_action(Some(&name[..]));

            let assign = match &free.action {
                FreeAction::Push(meth) => quote!{
                    let _arg = _free;
                    _result.#field.#meth(#parse);
                },
                FreeAction::SetField => quote!{
                    let _arg = _free;
                    _result.#field = #parse;
                },
                FreeAction::SetOption => quote!{
                    let _arg = _free;
                    _result.#field = ::std::option::Option::Some(#parse);
                },
            };

            quote!{
                #mark_used
                #assign
            }
        }).collect::<Vec<_>>();

        quote!{
            match _free_counter {
                #( #num => {
                    _free_counter += 1;
                    #action
                } )*
                _ => { #catch_all }
            }
        }
    } else if let Some(ident) = command {
        let mark_used = if command_required {
            quote!{ _used.#ident = true; }
        } else {
            quote!{ }
        };

        quote!{
            #mark_used
            _result.#ident = ::std::option::Option::Some(
                ::gumdrop::Options::parse_command(_free, _parser)?);
            break;
        }
    } else {
        quote!{
            return ::std::result::Result::Err(
                ::gumdrop::Error::unexpected_free(_free));
        }
    };

    let command_impl = match &command {
        None => quote!{ ::std::option::Option::None },
        Some(field) => quote!{
            ::std::option::Option::map(
                ::std::option::Option::as_ref(&self.#field),
                |sub| sub as _)
        }
    };

    let command_name_impl = match &command {
        None => quote!{ ::std::option::Option::None },
        Some(field) => quote!{
            ::std::option::Option::and_then(
                ::std::option::Option::as_ref(&self.#field),
                ::gumdrop::Options::command_name)
        }
    };

    let command_list = match command_ty {
        Some(ty) => quote!{
            ::std::option::Option::Some(
                <#ty as ::gumdrop::Options>::usage())
        },
        None => quote!{
            ::std::option::Option::None
        }
    };

    let command_usage = match command_ty {
        Some(ty) => quote!{
            <#ty as ::gumdrop::Options>::command_usage(_name)
        },
        None => quote!{
            ::std::option::Option::None
        }
    };

    let help_requested_impl = match (&help_flag, &command) {
        (flags, None) => quote!{
            fn help_requested(&self) -> bool {
                false #( || self.#flags )*
            }
        },
        (flags, Some(cmd)) => quote!{
            fn help_requested(&self) -> bool {
                #( self.#flags || )*
                ::std::option::Option::map_or(
                    ::std::option::Option::as_ref(&self.#cmd),
                    false, ::gumdrop::Options::help_requested)
            }
        }
    };

    let self_usage_impl = match &command {
        None => quote!{ <Self as ::gumdrop::Options>::usage() },
        Some(field) => quote!{
            ::std::option::Option::map_or_else(
                ::std::option::Option::as_ref(&self.#field),
                <Self as ::gumdrop::Options>::usage,
                ::gumdrop::Options::self_usage)
        }
    };

    let self_command_list_impl = match &command {
        None => quote!{ <Self as ::gumdrop::Options>::command_list() },
        Some(field) => quote!{
            ::std::option::Option::map_or_else(
                ::std::option::Option::as_ref(&self.#field),
                <Self as ::gumdrop::Options>::command_list,
                ::gumdrop::Options::self_command_list)
        }
    };

    let required = &required;

    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    Ok(quote!{
        impl #impl_generics ::gumdrop::Options for #name #ty_generics #where_clause {
            fn parse<__S: ::std::convert::AsRef<str>>(
                    _parser: &mut ::gumdrop::Parser<__S>)
                    -> ::std::result::Result<Self, ::gumdrop::Error> {
                #[derive(Default)]
                struct _Used {
                    #( #required: bool , )*
                }

                let mut _result = #name{
                    #( #field_name: #default ),*
                };
                let mut _free_counter = 0usize;
                let mut _used = _Used::default();

                while let ::std::option::Option::Some(_opt) = _parser.next_opt() {
                    match _opt {
                        #( #pattern => { #handle_opt } )*
                        ::gumdrop::Opt::Free(_free) => {
                            #handle_free
                        }
                        _ => {
                            return ::std::result::Result::Err(
                                ::gumdrop::Error::unrecognized_option(_opt));
                        }
                    }
                }

                if true #( && !_result.#help_flag )* {
                    #( if !_used.#required {
                        return ::std::result::Result::Err(#required_err);
                    } )*
                }

                ::std::result::Result::Ok(_result)
            }

            fn command(&self) -> ::std::option::Option<&dyn ::gumdrop::Options> {
                #command_impl
            }

            fn command_name(&self) -> ::std::option::Option<&'static str> {
                #command_name_impl
            }

            #help_requested_impl

            fn parse_command<__S: ::std::convert::AsRef<str>>(name: &str,
                    _parser: &mut ::gumdrop::Parser<__S>)
                    -> ::std::result::Result<Self, ::gumdrop::Error> {
                ::std::result::Result::Err(
                    ::gumdrop::Error::unrecognized_command(name))
            }

            fn usage() -> &'static str {
                #usage
            }

            fn self_usage(&self) -> &'static str {
                #self_usage_impl
            }

            fn command_list() -> ::std::option::Option<&'static str> {
                #command_list
            }

            fn command_usage(_name: &str) -> ::std::option::Option<&'static str> {
                #command_usage
            }

            fn self_command_list(&self) -> ::std::option::Option<&'static str> {
                #self_command_list_impl
            }
        }
    })
}

enum Action {
    /// Increase count
    Count,
    /// Push an argument to a `multi` field using the given method
    Push(Ident, ParseMethod),
    /// Set field
    SetField(ParseMethod),
    /// Set `Option<T>` field
    SetOption(ParseMethod),
    /// Set field to `true`
    Switch,
}

#[derive(Default)]
struct AttrOpts {
    long: Option<String>,
    short: Option<char>,
    multi: Option<Ident>,
    free: bool,
    count: bool,
    help_flag: bool,
    no_help_flag: bool,
    no_short: bool,
    no_long: bool,
    no_multi: bool,
    required: bool,
    not_required: bool,
    doc: Option<String>,
    help: Option<String>,
    meta: Option<String>,
    parse: Option<ParseFn>,
    default: Option<String>,
    #[cfg(feature = "default_expr")]
    default_expr: Option<Expr>,

    command: bool,
}

struct Cmd<'a> {
    name: String,
    help: Option<String>,
    variant_name: &'a Ident,
    ty: &'a Type,
}

#[derive(Default)]
struct CmdOpts {
    name: Option<String>,
    doc: Option<String>,
    help: Option<String>,
}

#[derive(Default)]
struct DefaultOpts {
    no_help_flag: bool,
    no_long: bool,
    no_multi: bool,
    no_short: bool,
    required: bool,
    doc: Option<String>,
    help: Option<String>,
}

enum FreeAction {
    Push(Ident),
    SetField,
    SetOption,
}

struct FreeOpt<'a> {
    field: &'a Ident,
    action: FreeAction,
    parse: ParseFn,
    required: bool,
    help: Option<String>,
}

struct Opt<'a> {
    field: &'a Ident,
    action: Action,
    long: Option<String>,
    short: Option<char>,
    no_short: bool,
    required: bool,
    help: Option<String>,
    meta: Option<String>,
    default: Option<String>,
    // NOTE: `default_expr` is not contained here
    // because it is not displayed to the user in usage text
}

#[derive(Clone)]
enum ParseFn {
    Default,
    FromStr(Option<Path>),
    TryFromStr(Path),
}

struct ParseMethod {
    parse_fn: ParseFn,
    tuple_len: Option<usize>,
}

impl Action {
    fn infer(ty: &Type, opts: &AttrOpts) -> Action {
        match ty {
            Type::Path(path) => {
                let path = path.path.segments.last().unwrap();
                let param = first_ty_param(ty);

                match &path.ident.to_string()[..] {
                    "bool" if opts.parse.is_none() => Action::Switch,
                    "Vec" if !opts.no_multi && param.is_some() => {
                        let tuple_len = tuple_len(param.unwrap());

                        Action::Push(
                            Ident::new("push", Span::call_site()),
                            ParseMethod{
                                parse_fn: opts.parse.clone().unwrap_or_default(),
                                tuple_len,
                            })
                    }
                    "Option" if param.is_some() => {
                        let tuple_len = tuple_len(param.unwrap());

                        Action::SetOption(ParseMethod{
                            parse_fn: opts.parse.clone().unwrap_or_default(),
                            tuple_len,
                        })
                    }
                    _ => {
                        if let Some(meth) = &opts.multi {
                            let tuple_len = param.and_then(tuple_len);

                            Action::Push(
                                meth.clone(),
                                ParseMethod{
                                    parse_fn: opts.parse.clone().unwrap_or_default(),
                                    tuple_len,
                                })
                        } else {
                            Action::SetField(ParseMethod{
                                parse_fn: opts.parse.clone().unwrap_or_default(),
                                tuple_len: tuple_len(ty),
                            })
                        }
                    }
                }
            }
            _ => {
                let tuple_len = tuple_len(ty);

                Action::SetField(ParseMethod{
                    parse_fn: opts.parse.clone().unwrap_or_default(),
                    tuple_len,
                })
            }
        }
    }

    fn takes_arg(&self) -> bool {
        use self::Action::*;

        match self {
            Push(_, parse) |
            SetField(parse) |
            SetOption(parse) => parse.takes_arg(),
            _ => false
        }
    }

    fn tuple_len(&self) -> Option<usize> {
        use self::Action::*;

        match self {
            Push(_, parse) |
            SetField(parse) |
            SetOption(parse) => parse.tuple_len,
            _ => None
        }
    }
}

impl AttrOpts {
    fn check(&self, span: Span) -> Result<(), Error> {
        macro_rules! err {
            ( $($tt:tt)* ) => { {
                return Err(Error::new(span, $($tt)*));
            } }
        }

        if self.command {
            if self.free { err!("`command` and `free` are mutually exclusive"); }
            if self.default.is_some() { err!("`command` and `default` are mutually exclusive"); }
            if self.multi.is_some() { err!("`command` and `multi` are mutually exclusive"); }
            if self.long.is_some() { err!("`command` and `long` are mutually exclusive"); }
            if self.short.is_some() { err!("`command` and `short` are mutually exclusive"); }
            if self.count { err!("`command` and `count` are mutually exclusive"); }
            if self.help_flag { err!("`command` and `help_flag` are mutually exclusive"); }
            if self.no_help_flag { err!("`command` and `no_help_flag` are mutually exclusive"); }
            if self.no_short { err!("`command` and `no_short` are mutually exclusive"); }
            if self.no_long { err!("`command` and `no_long` are mutually exclusive"); }
            if self.no_multi { err!("`command` and `no_multi` are mutually exclusive"); }
            if self.help.is_some() { err!("`command` and `help` are mutually exclusive"); }
            if self.meta.is_some() { err!("`command` and `meta` are mutually exclusive"); }
        }

        if self.free {
            if self.default.is_some() { err!("`free` and `default` are mutually exclusive"); }
            if self.long.is_some() { err!("`free` and `long` are mutually exclusive"); }
            if self.short.is_some() { err!("`free` and `short` are mutually exclusive"); }
            if self.count { err!("`free` and `count` are mutually exclusive"); }
            if self.help_flag { err!("`free` and `help_flag` are mutually exclusive"); }
            if self.no_help_flag { err!("`free` and `no_help_flag` are mutually exclusive"); }
            if self.no_short { err!("`free` and `no_short` are mutually exclusive"); }
            if self.no_long { err!("`free` and `no_long` are mutually exclusive"); }
            if self.meta.is_some() { err!("`free` and `meta` are mutually exclusive"); }
        }

        if self.multi.is_some() && self.no_multi {
            err!("`multi` and `no_multi` are mutually exclusive");
        }

        if self.help_flag && self.no_help_flag {
            err!("`help_flag` and `no_help_flag` are mutually exclusive");
        }

        if self.no_short && self.short.is_some() {
            err!("`no_short` and `short` are mutually exclusive");
        }

        if self.no_long && self.long.is_some() {
            err!("`no_long` and `long` are mutually exclusive");
        }

        if self.required && self.not_required {
            err!("`required` and `not_required` are mutually exclusive");
        }

        if self.parse.is_some() {
            if self.count { err!("`count` and `parse` are mutually exclusive"); }
        }

        #[cfg(feature = "default_expr")]
        {
            if self.default.is_some() && self.default_expr.is_some() {
                err!("`default` and `default_expr` are mutually exclusive");
            }
        }

        Ok(())
    }

    fn parse(span: Span, attrs: &[Attribute]) -> Result<AttrOpts, Error> {
        let mut opts = AttrOpts::default();

        for attr in attrs {
            if is_outer(attr.style) {
                if path_eq(&attr.path, "doc") {
                    let meta = attr.parse_meta()?;

                    if let Meta::NameValue(nv) = meta {
                        let doc = lit_str(&nv.lit)?;

                        if opts.doc.is_none() {
                            opts.doc = Some(doc.trim_start().to_owned());
                        }
                    }
                } else if path_eq(&attr.path, "options") {
                    let meta = attr.parse_meta()?;

                    match meta {
                        Meta::Path(path) =>
                            return Err(Error::new(path.span(),
                                "`#[options]` is not a valid attribute")),
                        Meta::NameValue(nv) =>
                            return Err(Error::new(nv.path.span(),
                                "`#[options = ...]` is not a valid attribute")),
                        Meta::List(items) => {
                            for item in &items.nested {
                                opts.parse_item(item)?;
                            }
                        }
                    }
                }
            }
        }

        opts.check(span)?;

        Ok(opts)
    }

    fn parse_item(&mut self, item: &NestedMeta) -> Result<(), Error> {
        match item {
            NestedMeta::Lit(lit) =>
                return Err(unexpected_meta_item(lit.span())),
            NestedMeta::Meta(item) => {
                match item {
                    Meta::Path(path) => match path.get_ident() {
                        Some(ident) => match ident.to_string().as_str() {
                            "free" => self.free = true,
                            "command" => self.command = true,
                            "count" => self.count = true,
                            "help_flag" => self.help_flag = true,
                            "no_help_flag" => self.no_help_flag = true,
                            "no_short" => self.no_short = true,
                            "no_long" => self.no_long = true,
                            "no_multi" => self.no_multi = true,
                            "required" => self.required = true,
                            "not_required" => self.not_required = true,
                            _ => return Err(unexpected_meta_item(path.span()))
                        }
                        None => return Err(unexpected_meta_item(path.span()))
                    },
                    Meta::List(list) => {
                        match list.path.get_ident() {
                            Some(ident) if ident.to_string() == "parse" => {
                                if list.nested.len() != 1 {
                                    return Err(unexpected_meta_item(list.path.span()));
                                }

                                self.parse = Some(ParseFn::parse(&list.nested[0])?);
                            }
                            _ => return Err(unexpected_meta_item(list.path.span()))
                        }
                    }
                    Meta::NameValue(nv) => {
                        match nv.path.get_ident() {
                            Some(ident) => match ident.to_string().as_str() {
                                "default" => self.default = Some(lit_str(&nv.lit)?),
                                #[cfg(feature = "default_expr")]
                                "default_expr" => {
                                    let expr = parse_str(&lit_str(&nv.lit)?)?;
                                    self.default_expr = Some(expr);
                                }
                                #[cfg(not(feature = "default_expr"))]
                                "default_expr" => {
                                    return Err(Error::new(nv.path.span(),
                                    "compile gumdrop with the `default_expr` \
                                    feature to enable this attribute"));
                                }
                                "long" => self.long = Some(lit_str(&nv.lit)?),
                                "short" => self.short = Some(lit_char(&nv.lit)?),
                                "help" => self.help = Some(lit_str(&nv.lit)?),
                                "meta" => self.meta = Some(lit_str(&nv.lit)?),
                                "multi" => {
                                    let name = parse_str(&lit_str(&nv.lit)?)?;
                                    self.multi = Some(name);
                                }
                                _ => return Err(unexpected_meta_item(nv.path.span()))
                            }
                            None => return Err(unexpected_meta_item(nv.path.span()))
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn set_defaults(&mut self, defaults: &DefaultOpts) {
        if !self.help_flag && defaults.no_help_flag {
            self.no_help_flag = true;
        }
        if self.short.is_none() && defaults.no_short {
            self.no_short = true;
        }
        if self.long.is_none() && defaults.no_long {
            self.no_long = true;
        }
        if self.multi.is_none() && defaults.no_multi {
            self.no_multi = true;
        }

        if self.not_required {
            self.required = false;
        } else if defaults.required {
            self.required = true;
        }
    }
}

impl CmdOpts {
    fn parse(attrs: &[Attribute]) -> Result<CmdOpts, Error> {
        let mut opts = CmdOpts::default();

        for attr in attrs {
            if is_outer(attr.style) {
                if path_eq(&attr.path, "doc") {
                    let meta = attr.parse_meta()?;

                    if let Meta::NameValue(nv) = meta {
                        let doc = lit_str(&nv.lit)?;

                        if opts.doc.is_none() {
                            opts.doc = Some(doc.trim_start().to_owned());
                        }
                    }
                } else if path_eq(&attr.path, "options") {
                    let meta = attr.parse_meta()?;

                    match meta {
                        Meta::Path(path) =>
                            return Err(Error::new(path.span(),
                                "`#[options]` is not a valid attribute")),
                        Meta::NameValue(nv) =>
                            return Err(Error::new(nv.path.span(),
                                "`#[options = ...]` is not a valid attribute")),
                        Meta::List(items) => {
                            for item in &items.nested {
                                opts.parse_item(item)?;
                            }
                        }
                    }
                }
            }
        }

        Ok(opts)
    }

    fn parse_item(&mut self, item: &NestedMeta) -> Result<(), Error> {
        match item {
            NestedMeta::Lit(lit) =>
                return Err(unexpected_meta_item(lit.span())),
            NestedMeta::Meta(item) => {
                match item {
                    Meta::Path(path) =>
                        return Err(unexpected_meta_item(path.span())),
                    Meta::List(list) =>
                        return Err(unexpected_meta_item(list.path.span())),
                    Meta::NameValue(nv) => {
                        match nv.path.get_ident() {
                            Some(ident) => match ident.to_string().as_str() {
                                "name" => self.name = Some(lit_str(&nv.lit)?),
                                "help" => self.help = Some(lit_str(&nv.lit)?),
                                _ => return Err(unexpected_meta_item(nv.path.span()))
                            }
                            None => return Err(unexpected_meta_item(nv.path.span()))
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

impl DefaultOpts {
    fn parse(attrs: &[Attribute]) -> Result<DefaultOpts, Error> {
        let mut opts = DefaultOpts::default();

        for attr in attrs {
            if is_outer(attr.style) {
                if path_eq(&attr.path, "doc") {
                    let meta = attr.parse_meta()?;

                    if let Meta::NameValue(nv) = meta {
                        let doc = lit_str(&nv.lit)?;

                        if let Some(text) = opts.doc.as_mut() {
                            text.push('\n');
                            text.push_str(doc.trim_start());
                        } else {
                            opts.doc = Some(doc.trim_start().to_owned());
                        }
                    }
                } else if path_eq(&attr.path, "options") {
                    let meta = attr.parse_meta()?;

                    match meta {
                        Meta::Path(path) =>
                            return Err(Error::new(path.span(),
                                "`#[options]` is not a valid attribute")),
                        Meta::NameValue(nv) =>
                            return Err(Error::new(nv.path.span(),
                                "`#[options = ...]` is not a valid attribute")),
                        Meta::List(items) => {
                            for item in &items.nested {
                                opts.parse_item(item)?;
                            }
                        }
                    }
                }
            }
        }

        Ok(opts)
    }

    fn parse_item(&mut self, item: &NestedMeta) -> Result<(), Error> {
        match item {
            NestedMeta::Lit(lit) =>
                return Err(unexpected_meta_item(lit.span())),
            NestedMeta::Meta(item) => {
                match item {
                    Meta::Path(path) => match path.get_ident() {
                        Some(ident) => match ident.to_string().as_str() {
                            "no_help_flag" => self.no_help_flag = true,
                            "no_short" => self.no_short = true,
                            "no_long" => self.no_long = true,
                            "no_multi" => self.no_multi = true,
                            "required" => self.required = true,
                            _ => return Err(unexpected_meta_item(ident.span()))
                        }
                        None => return Err(unexpected_meta_item(path.span()))
                    },
                    Meta::NameValue(nv) => {
                        match nv.path.get_ident() {
                           Some(ident) if ident.to_string() == "help" => self.help = Some(lit_str(&nv.lit)?),
                            _ => return Err(unexpected_meta_item(nv.path.span()))
                        }
                    }
                    Meta::List(list) =>
                        return Err(unexpected_meta_item(list.path.span()))
                }
            }
        }

        Ok(())
    }
}

impl FreeAction {
    fn infer(ty: &Type, opts: &AttrOpts) -> FreeAction {
        match ty {
            Type::Path(path) => {
                let path = path.path.segments.last().unwrap();

                match &path.ident.to_string()[..] {
                    "Option" => FreeAction::SetOption,
                    "Vec" if !opts.no_multi =>
                        FreeAction::Push(Ident::new("push", Span::call_site())),
                    _ => {
                        if let Some(meth) = &opts.multi {
                            FreeAction::Push(meth.clone())
                        } else {
                            FreeAction::SetField
                        }
                    }
                }
            }
            _ => FreeAction::SetField,
        }
    }

    fn is_push(&self) -> bool {
        match self {
            FreeAction::Push(_) => true,
            _ => false
        }
    }
}

impl<'a> FreeOpt<'a> {
    fn mark_used(&self) -> TokenStream2 {
        if self.required {
            let field = self.field;
            quote!{ _used.#field = true; }
        } else {
            quote!{ }
        }
    }

    fn width(&self) -> usize {
        2 + self.field.to_string().len() + 2 // name + spaces before and after
    }
}

impl<'a> Opt<'a> {
    fn display_form(&self) -> String {
        if let Some(long) = &self.long {
            format!("--{}", long)
        } else {
            format!("-{}", self.short.unwrap())
        }
    }

    fn mark_used(&self) -> TokenStream2 {
        if self.required {
            let field = self.field;
            quote!{ _used.#field = true; }
        } else {
            quote!{ }
        }
    }

    fn width(&self) -> usize {
        let short = self.short.map_or(0, |_| 1 + 1); // '-' + char
        let long = self.long.as_ref().map_or(0, |s| s.len() + 2); // "--" + str
        let sep = if short == 0 || long == 0 { 0 } else { 2 }; // ", "
        let meta = self.meta.as_ref().map_or(0, |s| s.len() + 1); // ' ' + meta

        2 + short + long + sep + meta + 2 // total + spaces before and after
    }

    fn make_action(&self) -> TokenStream2 {
        use self::Action::*;

        let field = self.field;
        let mark_used = self.mark_used();

        let action = match &self.action {
            Count => quote!{
                _result.#field += 1;
            },
            Push(meth, parse) => {
                let act = parse.make_action_type();

                quote!{
                    _result.#field.#meth(#act);
                }
            }
            SetField(parse) => {
                let act = parse.make_action_type();

                quote!{
                    _result.#field = #act;
                }
            }
            SetOption(parse) => {
                let act = parse.make_action_type();

                quote!{
                    _result.#field = ::std::option::Option::Some(#act);
                }
            }
            Switch => quote!{
                _result.#field = true;
            }
        };

        quote!{
            #mark_used
            #action
        }
    }

    fn make_action_arg(&self) -> TokenStream2 {
        use self::Action::*;

        let field = self.field;
        let mark_used = self.mark_used();

        let action = match &self.action {
            Push(meth, parse) => {
                let act = parse.make_action_type_arg();

                quote!{
                    _result.#field.#meth(#act);
                }
            }
            SetField(parse) => {
                let act = parse.make_action_type_arg();

                quote!{
                    _result.#field = #act;
                }
            }
            SetOption(parse) => {
                let act = parse.make_action_type_arg();

                quote!{
                    _result.#field = ::std::option::Option::Some(#act);
                }
            }
            _ => unreachable!()
        };

        quote!{
            #mark_used
            #action
        }
    }

    fn usage(&self, col_width: usize) -> String {
        let mut res = String::from("  ");

        if let Some(short) = self.short {
            res.push('-');
            res.push(short);
        }

        if self.short.is_some() && self.long.is_some() {
            res.push_str(", ");
        }

        if let Some(long) = &self.long {
            res.push_str("--");
            res.push_str(long);
        }

        if let Some(meta) = &self.meta {
            res.push(' ');
            res.push_str(meta);
        }

        if self.help.is_some() || self.default.is_some() {
            if res.len() < col_width {
                let n = col_width - res.len();
                res.extend(repeat(' ').take(n));
            } else {
                res.push('\n');
                res.extend(repeat(' ').take(col_width));
            }
        }

        if let Some(help) = &self.help {
            res.push_str(help);
        }

        if let Some(default) = &self.default {
            res.push_str(" (default: ");
            res.push_str(default);
            res.push_str(")");
        }

        res
    }
}

impl ParseFn {
    fn parse(item: &NestedMeta) -> Result<ParseFn, Error> {
        let result = match item {
            NestedMeta::Meta(Meta::Path(path)) => {
                match path.get_ident() {
                    Some(ident) => match ident.to_string().as_str() {
                        "from_str" => ParseFn::FromStr(None),
                        "try_from_str" => ParseFn::Default,
                        _ => return Err(unexpected_meta_item(ident.span()))
                    }
                    None => return Err(unexpected_meta_item(path.span()))
                }
            }
            NestedMeta::Meta(Meta::NameValue(nv)) => {
                match nv.path.get_ident() {
                    Some(ident) => match ident.to_string().as_str() {
                        "from_str" => {
                            let path = parse_str(&lit_str(&nv.lit)?)?;
                            ParseFn::FromStr(Some(path))
                        }
                        "try_from_str" => {
                            let path = parse_str(&lit_str(&nv.lit)?)?;
                            ParseFn::TryFromStr(path)
                        }
                        _ => return Err(unexpected_meta_item(nv.path.span()))
                    }
                    None => return Err(unexpected_meta_item(nv.path.span()))
                }
            }
            NestedMeta::Lit(_) |
            NestedMeta::Meta(Meta::List(_)) =>
                return Err(unexpected_meta_item(item.span())),
        };

        Ok(result)
    }

    fn make_parse_action(&self, name: Option<&str>) -> TokenStream2 {
        let name = if let Some(name) = name {
            quote!{ ::std::string::ToString::to_string(#name) }
        } else {
            quote!{ ::gumdrop::Opt::to_string(&_opt) }
        };

        let res = match self {
            ParseFn::Default => quote!{
                ::std::str::FromStr::from_str(_arg)
                    .map_err(|e| ::gumdrop::Error::failed_parse_with_name(
                        #name, ::std::string::ToString::to_string(&e)))?
            },
            ParseFn::FromStr(None) => quote!{
                ::std::convert::From::from(_arg)
            },
            ParseFn::FromStr(Some(fun)) => quote!{
                #fun(_arg)
            },
            ParseFn::TryFromStr(fun) => quote!{
                #fun(_arg)
                    .map_err(|e| ::gumdrop::Error::failed_parse_with_name(
                        #name, ::std::string::ToString::to_string(&e)))?
            }
        };

        res
    }

    fn make_parse_default_action(&self, ident: &Ident, expr: &str) -> TokenStream2 {
        let res = match self {
            ParseFn::Default => quote!{
                ::std::str::FromStr::from_str(#expr)
                    .map_err(|e| ::gumdrop::Error::failed_parse_default(
                        stringify!(#ident), #expr,
                        ::std::string::ToString::to_string(&e)))?
            },
            ParseFn::FromStr(None) => quote!{
                ::std::convert::From::from(#expr)
            },
            ParseFn::FromStr(Some(fun)) => quote!{
                #fun(#expr)
            },
            ParseFn::TryFromStr(fun) => quote!{
                #fun(#expr)
                    .map_err(|e| ::gumdrop::Error::failed_parse_default(
                        stringify!(#ident), #expr,
                        ::std::string::ToString::to_string(&e)))?
            }
        };

        res
    }
}

impl Default for ParseFn {
    fn default() -> ParseFn {
        ParseFn::Default
    }
}

impl ParseMethod {
    fn make_action_type(&self) -> TokenStream2 {
        let parse = self.parse_fn.make_parse_action(None);

        match self.tuple_len {
            None => quote!{ {
                let _arg = _parser.next_arg()
                    .ok_or_else(|| ::gumdrop::Error::missing_argument(_opt))?;

                #parse
            } },
            Some(n) => {
                let num = 0..n;
                let n = repeat(n);
                let parse = repeat(parse);

                quote!{
                    ( #( {
                        let _found = #num;
                        let _arg = _parser.next_arg()
                            .ok_or_else(|| ::gumdrop::Error::insufficient_arguments(
                                _opt, #n, _found))?;

                        #parse
                    } , )* )
                }
            }
        }
    }

    fn make_action_type_arg(&self) -> TokenStream2 {
        match self.tuple_len {
            None => self.parse_fn.make_parse_action(None),
            Some(_) => unreachable!()
        }
    }
    fn takes_arg(&self) -> bool {
        match self.tuple_len {
            Some(0) => false,
            _ => true
        }
    }
}

fn first_ty_param(ty: &Type) -> Option<&Type> {
    match ty {
        Type::Path(path) => {
            let path = path.path.segments.last().unwrap();

            match &path.arguments {
                PathArguments::AngleBracketed(data) =>
                    data.args.iter().filter_map(|arg| match arg {
                        GenericArgument::Type(ty) => Some(ty),
                        _ => None
                    }).next(),
                _ => None
            }
        }
        _ => None
    }
}

fn is_outer(style: AttrStyle) -> bool {
    match style {
        AttrStyle::Outer => true,
        _ => false
    }
}

fn lit_str(lit: &Lit) -> Result<String, Error> {
    match lit {
        Lit::Str(s) => Ok(s.value()),
        _ => Err(Error::new(lit.span(), "expected string literal"))
    }
}

fn lit_char(lit: &Lit) -> Result<char, Error> {
    match lit {
        Lit::Char(ch) => Ok(ch.value()),
        // Character literals in attributes are not necessarily allowed
        Lit::Str(s) => {
            let s = s.value();
            let mut chars = s.chars();

            let first = chars.next();
            let second = chars.next();

            match (first, second) {
                (Some(ch), None) => Ok(ch),
                _ => Err(Error::new(lit.span(),
                    "expected one-character string literal"))
            }
        }
        _ => Err(Error::new(lit.span(), "expected character literal"))
    }
}

fn path_eq(path: &Path, s: &str) -> bool {
    path.segments.len() == 1 && {
        let seg = path.segments.first().unwrap();

        match seg.arguments {
            PathArguments::None => seg.ident == s,
            _ => false
        }
    }
}

fn tuple_len(ty: &Type) -> Option<usize> {
    match ty {
        Type::Tuple(tup) => Some(tup.elems.len()),
        _ => None
    }
}

fn make_command_name(name: &str) -> String {
    let mut res = String::with_capacity(name.len());

    for ch in name.chars() {
        if ch.is_lowercase() {
            res.push(ch);
        } else {
            if !res.is_empty() {
                res.push('-');
            }

            res.extend(ch.to_lowercase());
        }
    }

    res
}

fn make_long_name(name: &str) -> String {
    name.replace('_', "-")
}

fn make_short_name(name: &str, short: &[char]) -> Option<char> {
    let first = name.chars().next().expect("empty field name");

    if !short.contains(&first) {
        return Some(first);
    }

    let mut to_upper = first.to_uppercase();
    let upper = to_upper.next().expect("empty to_uppercase");

    if to_upper.next().is_some() {
        return None;
    }

    if !short.contains(&upper) {
        Some(upper)
    } else {
        None
    }
}

fn validate_long_name(span: Span, name: &str, names: &[String])
        -> Result<(), Error> {
    if name.is_empty() || name.starts_with('-') ||
            name.contains(char::is_whitespace) {
        Err(Error::new(span, "not a valid long option"))
    } else if names.iter().any(|n| n == name) {
        Err(Error::new(span, "duplicate option name"))
    } else {
        Ok(())
    }
}

fn validate_short_name(span: Span, ch: char, names: &[char])
        -> Result<(), Error> {
    if ch == '-' || ch.is_whitespace() {
        Err(Error::new(span, "not a valid short option"))
    } else if names.contains(&ch) {
        Err(Error::new(span, "duplicate option name"))
    } else {
        Ok(())
    }
}

fn make_meta(name: &str, action: &Action) -> String {
    use std::fmt::Write;

    let mut name = name.replace('_', "-").to_uppercase();

    match action.tuple_len() {
        Some(0) => unreachable!(),
        Some(1) | None => (),
        Some(2) => {
            name.push_str(" VALUE");
        }
        Some(n) => {
            for i in 1..n {
                let _ = write!(name, " VALUE{}", i - 1);
            }
        }
    }

    name
}

fn make_usage(help: &Option<String>, free: &[FreeOpt], opts: &[Opt]) -> String {
    let mut res = String::new();

    if let Some(help) = help {
        res.push_str(help);
        res.push('\n');
    }

    let width = max_width(free, |opt| opt.width())
        .max(max_width(opts, |opt| opt.width()));

    if !free.is_empty() {
        if !res.is_empty() {
            res.push('\n');
        }

        res.push_str("Positional arguments:\n");

        for opt in free {
            let mut line = String::from("  ");

            line.push_str(&opt.field.to_string());

            if let Some(help) = &opt.help {
                if line.len() < width {
                    let n = width - line.len();
                    line.extend(repeat(' ').take(n));
                } else {
                    line.push('\n');
                    line.extend(repeat(' ').take(width));
                }

                line.push_str(help);
            }

            res.push_str(&line);
            res.push('\n');
        }
    }

    if !opts.is_empty() {
        if !res.is_empty() {
            res.push('\n');
        }

        res.push_str("Optional arguments:\n");

        for opt in opts {
            res.push_str(&opt.usage(width));
            res.push('\n');
        }
    }

    // Pop the last newline so the user may println!() the result.
    res.pop();

    res
}

fn max_width<T, F>(items: &[T], f: F) -> usize
        where F: Fn(&T) -> usize {
    const MIN_WIDTH: usize = 8;
    const MAX_WIDTH: usize = 30;

    let width = items.iter().filter_map(|item| {
        let w = f(item);

        if w > MAX_WIDTH {
            None
        } else {
            Some(w)
        }
    }).max().unwrap_or(0);

    width.max(MIN_WIDTH).min(MAX_WIDTH)
}

fn make_cmd_usage(cmds: &[Cmd]) -> String {
    let mut res = String::new();

    let width = max_width(cmds,
        // Two spaces each, before and after
        |cmd| cmd.name.len() + 4);

    for cmd in cmds {
        let mut line = String::from("  ");

        line.push_str(&cmd.name);

        if let Some(help) = &cmd.help {
            if line.len() < width {
                let n = width - line.len();
                line.extend(repeat(' ').take(n));
            } else {
                line.push('\n');
                line.extend(repeat(' ').take(width));
            }

            line.push_str(help);
        }

        res.push_str(&line);
        res.push('\n');
    }

    // Pop the last newline
    res.pop();

    res
}

fn unexpected_meta_item(span: Span) -> Error {
    Error::new(span, "unexpected meta item")
}
