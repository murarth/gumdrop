//! Option parser with custom derive support
//!
//! For full documentation on customization of `derive(Options)`, please see the crate
//! documentation for [`gumdrop_derive`](https://docs.rs/crate/gumdrop_derive/).
//!
//! # Examples
//!
//! ```
//! use gumdrop::Options;
//!
//! // Defines options that can be parsed from the command line.
//! //
//! // `derive(Options)` will generate an implementation of the trait `Options`.
//! // Each field must either have a `Default` implementation or an inline
//! // default value provided.
//! //
//! // (`Debug` is derived here only for demonstration purposes.)
//! #[derive(Debug, Options)]
//! struct MyOptions {
//!     // Contains "free" arguments -- those that are not options.
//!     // If no `free` field is declared, free arguments will result in an error.
//!     #[options(free)]
//!     free: Vec<String>,
//!
//!     // Boolean options are treated as flags, taking no additional values.
//!     // The optional `help` attribute is displayed in `usage` text.
//!     //
//!     // A boolean field named `help` is automatically given the `help_flag` attribute.
//!     // The `parse_args_or_exit` and `parse_args_default_or_exit` functions use help flags
//!     // to automatically display usage to the user.
//!     #[options(help = "print help message")]
//!     help: bool,
//!
//!     // Non-boolean fields will take a value from the command line.
//!     // Wrapping the type in an `Option` is not necessary, but provides clarity.
//!     #[options(help = "give a string argument")]
//!     string: Option<String>,
//!
//!     // A field can be any type that implements `FromStr`.
//!     // The optional `meta` attribute is displayed in `usage` text.
//!     #[options(help = "give a number as an argument", meta = "N")]
//!     number: Option<i32>,
//!
//!     // A `Vec` field will accumulate all values received from the command line.
//!     #[options(help = "give a list of string items")]
//!     item: Vec<String>,
//!
//!     // The `count` flag will treat the option as a counter.
//!     // Each time the option is encountered, the field is incremented.
//!     #[options(count, help = "increase a counting value")]
//!     count: u32,
//!
//!     // Option names are automatically generated from field names, but these
//!     // can be overriden. The attributes `short = "?"`, `long = "..."`,
//!     // `no_short`, and `no_long` are used to control option names.
//!     #[options(no_short, help = "this option has no short form")]
//!     long_option_only: bool,
//! }
//!
//! fn main() {
//!     let opts = MyOptions::parse_args_default_or_exit();
//!
//!     println!("{:#?}", opts);
//! }
//! ```
//!
//! `derive(Options)` can also be used on `enum`s to produce a subcommand
//! option parser.
//!
//! ```
//! use gumdrop::Options;
//!
//! // Define options for the program.
//! #[derive(Debug, Options)]
//! struct MyOptions {
//!     // Options here can be accepted with any command (or none at all),
//!     // but they must come before the command name.
//!     #[options(help = "print help message")]
//!     help: bool,
//!     #[options(help = "be verbose")]
//!     verbose: bool,
//!
//!     // The `command` option will delegate option parsing to the command type,
//!     // starting at the first free argument.
//!     #[options(command)]
//!     command: Option<Command>,
//! }
//!
//! // The set of commands and the options each one accepts.
//! //
//! // Each variant of a command enum should be a unary tuple variant with only
//! // one field. This field must implement `Options` and is used to parse arguments
//! // that are given after the command name.
//! #[derive(Debug, Options)]
//! enum Command {
//!     // Command names are generated from variant names.
//!     // By default, a CamelCase name will be converted into a lowercase,
//!     // hyphen-separated name; e.g. `FooBar` becomes `foo-bar`.
//!     //
//!     // Names can be explicitly specified using `#[options(name = "...")]`
//!     #[options(help = "show help for a command")]
//!     Help(HelpOpts),
//!     #[options(help = "make stuff")]
//!     Make(MakeOpts),
//!     #[options(help = "install stuff")]
//!     Install(InstallOpts),
//! }
//!
//! // Options accepted for the `help` command
//! #[derive(Debug, Options)]
//! struct HelpOpts {
//!     #[options(free)]
//!     free: Vec<String>,
//! }
//!
//! // Options accepted for the `make` command
//! #[derive(Debug, Options)]
//! struct MakeOpts {
//!     #[options(free)]
//!     free: Vec<String>,
//!     #[options(help = "number of jobs", meta = "N")]
//!     jobs: Option<u32>,
//! }
//!
//! // Options accepted for the `install` command
//! #[derive(Debug, Options)]
//! struct InstallOpts {
//!     #[options(help = "target directory")]
//!     dir: Option<String>,
//! }
//!
//! fn main() {
//!     let opts = MyOptions::parse_args_default_or_exit();
//!
//!     println!("{:#?}", opts);
//! }
//! ```
//!
//! A custom parsing function can be supplied for each option field.
//!
//! ```
//! use gumdrop::Options;
//!
//! #[derive(Debug, Options)]
//! struct MyOptions {
//!     // `try_from_str = "..."` supplies a conversion function that may fail
//!     #[options(help = "a hexadecimal value", parse(try_from_str = "parse_hex"))]
//!     hex: u32,
//!     // `from_str = "..."` supplies a conversion function that always succeeds
//!     #[options(help = "a string that becomes uppercase", parse(from_str = "to_upper"))]
//!     upper: String,
//! }
//!
//! fn parse_hex(s: &str) -> Result<u32, std::num::ParseIntError> {
//!     u32::from_str_radix(s, 16)
//! }
//!
//! fn to_upper(s: &str) -> String {
//!     s.to_uppercase()
//! }
//!
//! fn main() {
//!     let opts = MyOptions::parse_args_default_or_exit();
//!
//!     println!("{:#?}", opts);
//! }
//! ```

#![deny(missing_docs)]

pub use gumdrop_derive::*;

use std::error::Error as StdError;
use std::fmt;
use std::slice::Iter;
use std::str::Chars;

/// Represents an error encountered during argument parsing
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
}

#[derive(Debug)]
enum ErrorKind {
    FailedParse(String, String),
    FailedParseDefault{
        option: &'static str,
        value: &'static str,
        err: String,
    },
    InsufficientArguments{
        option: String,
        expected: usize,
        found: usize,
    },
    MissingArgument(String),
    MissingCommand,
    MissingRequired(String),
    MissingRequiredCommand,
    MissingRequiredFree,
    UnexpectedArgument(String),
    UnexpectedSingleArgument(String, usize),
    UnexpectedFree(String),
    UnrecognizedCommand(String),
    UnrecognizedLongOption(String),
    UnrecognizedShortOption(char),
}

/// Parses options from a series of `&str`-like values.
pub struct Parser<'a, S: 'a> {
    args: Iter<'a, S>,
    cur: Option<Chars<'a>>,
    style: ParsingStyle,
    terminated: bool,
}

/// Represents an option parsed from a `Parser`
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Opt<'a> {
    /// Short option, e.g. `-o`
    Short(char),
    /// Long option, e.g. `--option`
    Long(&'a str),
    /// Long option with argument, e.g. `--option=value`
    LongWithArg(&'a str, &'a str),
    /// Free argument
    Free(&'a str),
}

/// Implements a set of options parsed from command line arguments.
///
/// An implementation of this trait can be generated with `#[derive(Options)]`.
pub trait Options {
    /// Parses arguments until the given parser is exhausted or until
    /// an error is encountered.
    fn parse<S: AsRef<str>>(parser: &mut Parser<S>) -> Result<Self, Error> where Self: Sized;

    /// Returns the subcommand instance, if present.
    ///
    /// This method **must never** return `self` or otherwise return a `&dyn Options` instance
    /// which would create a cycle. Doing so may cause other methods or `gumdrop` functions
    /// to loop infinitely or overflow the runtime stack.
    fn command(&self) -> Option<&dyn Options>;

    /// Returns the name of a parsed command, if present.
    ///
    /// This is implemented by `derive(Options)` in one of two ways:
    ///
    /// * For `struct` types, if the type contains a field marked
    ///   `#[options(command)]`, this method is called on that value.
    ///   Otherwise, `None` is returned.
    /// * For `enum` types, the name corresponding to the variant is returned.
    fn command_name(&self) -> Option<&'static str> { None }

    /// Returns whether the user supplied a "help" option to request
    /// usage information about the program or any contained subcommands.
    ///
    /// The default implementation returns `false`.
    fn help_requested(&self) -> bool { false }

    /// Parses arguments received from the command line.
    ///
    /// The first argument (the program name) should be omitted.
    fn parse_args<S: AsRef<str>>(args: &[S], style: ParsingStyle) -> Result<Self, Error>
            where Self: Sized {
        Self::parse(&mut Parser::new(args, style))
    }

    /// Parses arguments from the environment.
    ///
    /// If an error is encountered, the error is printed to `stderr` and the
    /// process will exit with status code `2`.
    ///
    /// If the user supplies a help option, option usage will be printed to
    /// `stdout` and the process will exit with status code `0`.
    ///
    /// Otherwise, the parsed options are returned.
    fn parse_args_or_exit(style: ParsingStyle) -> Self where Self: Sized {
        use std::env::args;
        use std::process::exit;

        let args = args().collect::<Vec<_>>();

        let opts = Self::parse_args(&args[1..], style).unwrap_or_else(|e| {
            eprintln!("{}: {}", args[0], e);
            exit(2);
        });

        if opts.help_requested() {
            let mut command = &opts as &dyn Options;
            let mut command_str = String::new();

            loop {
                if let Some(new_command) = command.command() {
                    command = new_command;

                    if let Some(name) = new_command.command_name() {
                        command_str.push(' ');
                        command_str.push_str(name);
                    }
                } else {
                    break;
                }
            }

            println!("Usage: {}{} [OPTIONS]", args[0], command_str);
            println!();
            println!("{}", command.self_usage());

            if let Some(cmds) = command.self_command_list() {
                println!();
                println!("Available commands:");
                println!("{}", cmds);
            }

            exit(0);
        }

        opts
    }

    /// Parses arguments from the environment, using the default
    /// [parsing style](enum.ParsingStyle.html).
    ///
    /// If an error is encountered, the error is printed to `stderr` and the
    /// process will exit with status code `2`.
    ///
    /// If the user supplies a help option, option usage will be printed to
    /// `stdout` and the process will exit with status code `0`.
    ///
    /// Otherwise, the parsed options are returned.
    fn parse_args_default_or_exit() -> Self where Self: Sized {
        Self::parse_args_or_exit(ParsingStyle::default())
    }

    /// Parses arguments received from the command line,
    /// using the default [parsing style](enum.ParsingStyle.html).
    ///
    /// The first argument (the program name) should be omitted.
    fn parse_args_default<S: AsRef<str>>(args: &[S]) -> Result<Self, Error> where Self: Sized {
        Self::parse(&mut Parser::new(args, ParsingStyle::default()))
    }

    /// Parses options for the named command.
    fn parse_command<S: AsRef<str>>(name: &str, parser: &mut Parser<S>) -> Result<Self, Error> where Self: Sized;

    /// Returns a string showing usage and help for each supported option.
    ///
    /// Option descriptions are separated by newlines. The returned string
    /// should **not** end with a newline.
    fn usage() -> &'static str where Self: Sized;

    /// Returns a string showing usage and help for this options instance.
    ///
    /// In contrast to `usage`, this method will return usage for a subcommand,
    /// if one is selected.
    ///
    /// Option descriptions are separated by newlines. The returned string
    /// should **not** end with a newline.
    fn self_usage(&self) -> &'static str;

    /// Returns a usage string for the named command.
    ///
    /// If the named command does not exist, `None` is returned.
    ///
    /// Command descriptions are separated by newlines. The returned string
    /// should **not** end with a newline.
    fn command_usage(command: &str) -> Option<&'static str> where Self: Sized;

    /// Returns a string listing available commands and help text.
    ///
    /// Commands are separated by newlines. The string should **not** end with
    /// a newline.
    ///
    /// For `enum` types with `derive(Options)`, this is the same as `usage`.
    ///
    /// For `struct` types containing a field marked `#[options(command)]`,
    /// `usage` is called on the command type.
    fn command_list() -> Option<&'static str> where Self: Sized;

    /// Returns a listing of available commands and help text.
    ///
    /// In contrast to `usage`, this method will return command list for a subcommand,
    /// if one is selected.
    ///
    /// Commands are separated by newlines. The string should **not** end with
    /// a newline.
    fn self_command_list(&self) -> Option<&'static str>;
}

/// Implements a set of options parsed from command line arguments.
///
/// An implementation of this trait can be generated with `#[derive(OptionsCore)]`.
/// Unlike the `Options` trait, OptionsCore has fewer features,
/// and a few changes to its parsing. The primary differences are:
/// - OptionsCore does not have usage
/// - OptionsCore has no methods for printing usage/exiting
/// - OptionsCore does not treat unknown options as errors
/// - OptionsCore does not treat extra free positional arguments as errors
/// - OptionsCore does not implicitly treat "-h" or "--help" as a special variable
/// - OptionsCore does not use the first letter of a field as the short form
pub trait OptionsCore {
    /// Parses arguments until the given parser is exhausted or until
    /// an error is encountered.
    /// This method is auto-implemented if you use the #[derive(OptionsCore)]
    fn parse<S: AsRef<str>>(parser: &mut Parser<S>) -> Result<Self, Error> where Self: Sized;

    /// Parses options for the named command.
    /// This method is auto-implemented if you use the #[derive(OptionsCore)]
    fn parse_command<S: AsRef<str>>(name: &str, parser: &mut Parser<S>) -> Result<Self, Error> where Self: Sized;

    /// Parses arguments received from the command line.
    ///
    /// The first argument (the program name) should be omitted.
    fn parse_args<S: AsRef<str>>(args: &[S], style: ParsingStyle) -> Result<Self, Error>
            where Self: Sized {
        Self::parse(&mut Parser::new(args, style))
    }

    /// Parses arguments received from the command line,
    /// using the default [parsing style](enum.ParsingStyle.html).
    ///
    /// The first argument (the program name) should be omitted.
    fn parse_args_default<S: AsRef<str>>(args: &[S]) -> Result<Self, Error> where Self: Sized {
        Self::parse(&mut Parser::new(args, ParsingStyle::default()))
    }
}

/// Controls behavior of free arguments in `Parser`
///
/// The [`parse_args_default`] and [`parse_args_default_or_exit`] functions will use the
/// default parsing style, `AllOptions`.
///
/// # Examples
///
/// ```
/// use gumdrop::{Options, ParsingStyle};
///
/// #[derive(Options)]
/// struct MyOptions {
///     // If the "-o" is parsed as an option, this will be `true`.
///     option: bool,
///     // All free (non-option) arguments will be collected into this Vec.
///     #[options(free)]
///     free: Vec<String>,
/// }
///
/// // Command line arguments.
/// let args = &["foo", "-o", "bar"];
///
/// // Using the `AllOptions` parsing style, the "-o" argument in the middle of args
/// // will be parsed as an option.
/// let opts = MyOptions::parse_args(args, ParsingStyle::AllOptions).unwrap();
///
/// assert_eq!(opts.option, true);
/// assert_eq!(opts.free, vec!["foo", "bar"]);
///
/// // Using the `StopAtFirstFree` option, the first non-option argument will terminate
/// // option parsing. That means "-o" is treated as a free argument.
/// let opts = MyOptions::parse_args(args, ParsingStyle::StopAtFirstFree).unwrap();
///
/// assert_eq!(opts.option, false);
/// assert_eq!(opts.free, vec!["foo", "-o", "bar"]);
/// ```
///
/// [`parse_args_default`]: fn.parse_args_default.html
/// [`parse_args_default_or_exit`]: fn.parse_args_default_or_exit.html
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ParsingStyle {
    /// Process all option arguments that appear
    AllOptions,
    /// After the first "free" argument is encountered,
    /// all remaining arguments will be considered "free" arguments.
    StopAtFirstFree,
}

impl Error {
    /// Returns an error for a failed attempt at parsing an option value.
    pub fn failed_parse(opt: Opt, err: String) -> Error {
        Error{kind: ErrorKind::FailedParse(opt.to_string(), err)}
    }

    /// Returns an error for a failed attempt at parsing an option's default value.
    pub fn failed_parse_default(option: &'static str,
            value: &'static str, err: String) -> Error {
        Error{kind: ErrorKind::FailedParseDefault{option, value, err}}
    }

    /// Returns an error for a failed attempt at parsing an option value.
    pub fn failed_parse_with_name(name: String, err: String) -> Error {
        Error{kind: ErrorKind::FailedParse(name, err)}
    }

    /// Returns an error for an option expecting two or more arguments not
    /// receiving the expected number of arguments.
    pub fn insufficient_arguments(opt: Opt, expected: usize, found: usize) -> Error {
        Error{kind: ErrorKind::InsufficientArguments{
            option: opt.to_string(),
            expected: expected,
            found: found,
        }}
    }

    /// Returns an error for an option receiving an unexpected argument value,
    /// e.g. `--option=value`.
    pub fn unexpected_argument(opt: Opt) -> Error {
        Error{kind: ErrorKind::UnexpectedArgument(opt.to_string())}
    }

    /// Returns an error for an option expecting two or more argument values
    /// receiving only one in the long form, e.g. `--option=value`.
    ///
    /// These options must be passed as, e.g. `--option value second-value [...]`.
    pub fn unexpected_single_argument(opt: Opt, n: usize) -> Error {
        Error{kind: ErrorKind::UnexpectedSingleArgument(opt.to_string(), n)}
    }

    /// Returns an error for a missing required argument.
    pub fn missing_argument(opt: Opt) -> Error {
        Error{kind: ErrorKind::MissingArgument(opt.to_string())}
    }

    /// Returns an error for a missing command name.
    pub fn missing_command() -> Error {
        Error{kind: ErrorKind::MissingCommand}
    }

    /// Returns an error for a missing required option.
    pub fn missing_required(opt: &str) -> Error {
        Error{kind: ErrorKind::MissingRequired(opt.to_owned())}
    }

    /// Returns an error for a missing required command.
    pub fn missing_required_command() -> Error {
        Error{kind: ErrorKind::MissingRequiredCommand}
    }

    /// Returns an error for a missing required free argument.
    pub fn missing_required_free() -> Error {
        Error{kind: ErrorKind::MissingRequiredFree}
    }

    /// Returns an error when a free argument was encountered, but the options
    /// type does not support free arguments.
    pub fn unexpected_free(arg: &str) -> Error {
        Error{kind: ErrorKind::UnexpectedFree(arg.to_owned())}
    }

    /// Returns an error for an unrecognized command.
    pub fn unrecognized_command(name: &str) -> Error {
        Error{kind: ErrorKind::UnrecognizedCommand(name.to_owned())}
    }

    /// Returns an error for an unrecognized option.
    pub fn unrecognized_option(opt: Opt) -> Error {
        match opt {
            Opt::Short(short) => Error::unrecognized_short(short),
            Opt::Long(long) | Opt::LongWithArg(long, _) =>
                Error::unrecognized_long(long),
            Opt::Free(_) => panic!("`Error::unrecognized_option` called with `Opt::Free` value")
        }
    }

    /// Returns an error for an unrecognized long option, e.g. `--option`.
    pub fn unrecognized_long(opt: &str) -> Error {
        Error{kind: ErrorKind::UnrecognizedLongOption(opt.to_owned())}
    }

    /// Returns an error for an unrecognized short option, e.g. `-o`.
    pub fn unrecognized_short(opt: char) -> Error {
        Error{kind: ErrorKind::UnrecognizedShortOption(opt)}
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ErrorKind::*;

        match &self.kind {
            FailedParse(opt, arg) => write!(f, "invalid argument to option `{}`: {}", opt, arg),
            FailedParseDefault{option, value, err} => write!(f, "invalid default value for `{}` ({:?}): {}", option, value, err),
            InsufficientArguments{option, expected, found} =>
                write!(f, "insufficient arguments to option `{}`: expected {}; found {}",
                    option, expected, found),
            MissingArgument(opt) => write!(f, "missing argument to option `{}`", opt),
            MissingCommand => f.write_str("missing command name"),
            MissingRequired(opt) => write!(f, "missing required option `{}`", opt),
            MissingRequiredCommand => f.write_str("missing required command"),
            MissingRequiredFree => f.write_str("missing required free argument"),
            UnexpectedArgument(opt) => write!(f, "option `{}` does not accept an argument", opt),
            UnexpectedSingleArgument(opt, n) =>
                write!(f, "option `{}` expects {} arguments; found 1", opt, n),
            UnexpectedFree(arg) => write!(f, "unexpected free argument `{}`", arg),
            UnrecognizedCommand(cmd) => write!(f, "unrecognized command `{}`", cmd),
            UnrecognizedLongOption(opt) => write!(f, "unrecognized option `--{}`", opt),
            UnrecognizedShortOption(opt) => write!(f, "unrecognized option `-{}`", opt),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        "failed to parse arguments"
    }
}

impl<'a, S: 'a + AsRef<str>> Parser<'a, S> {
    /// Returns a new parser for the given series of arguments.
    ///
    /// The given slice should **not** contain the program name as its first
    /// element.
    pub fn new(args: &'a [S], style: ParsingStyle) -> Parser<'a, S> {
        Parser{
            args: args.iter(),
            cur: None,
            style: style,
            terminated: false,
        }
    }

    /// Returns the next option or `None` if no options remain.
    pub fn next_opt(&mut self) -> Option<Opt<'a>> {
        if let Some(mut cur) = self.cur.take() {
            if let Some(opt) = cur.next() {
                self.cur = Some(cur);
                return Some(Opt::Short(opt));
            }
        }

        if self.terminated {
            return self.args.next().map(|s| Opt::Free(s.as_ref()));
        }

        match self.args.next().map(|s| s.as_ref()) {
            Some(arg @ "-") => {
                if self.style == ParsingStyle::StopAtFirstFree {
                    self.terminated = true;
                }
                Some(Opt::Free(arg))
            }
            Some("--") => {
                self.terminated = true;
                self.args.next().map(|s| Opt::Free(s.as_ref()))
            }
            Some(long) if long.starts_with("--") => {
                match long.find('=') {
                    Some(pos) => Some(Opt::LongWithArg(
                        &long[2..pos], &long[pos + 1..])),
                    None => Some(Opt::Long(&long[2..]))
                }
            }
            Some(short) if short.starts_with('-') => {
                let mut chars = short[1..].chars();

                let res = chars.next().map(Opt::Short);

                self.cur = Some(chars);
                res
            }
            Some(free) => {
                if self.style == ParsingStyle::StopAtFirstFree {
                    self.terminated = true;
                }
                Some(Opt::Free(free))
            }
            None => None
        }
    }

    /// Returns the next argument to an option or `None` if none remain.
    pub fn next_arg(&mut self) -> Option<&'a str> {
        if let Some(cur) = self.cur.take() {
            let arg = cur.as_str();

            if !arg.is_empty() {
                return Some(arg);
            }
        }

        self.args.next().map(|s| s.as_ref())
    }
}

impl<'a, S: 'a> Clone for Parser<'a, S> {
    fn clone(&self) -> Parser<'a, S> {
        Parser{
            args: self.args.clone(),
            cur: self.cur.clone(),
            style: self.style,
            terminated: self.terminated,
        }
    }
}

impl<'a> Opt<'a> {
    #[doc(hidden)]
    pub fn to_string(&self) -> String {
        match *self {
            Opt::Short(ch) => format!("-{}", ch),
            Opt::Long(s) => format!("--{}", s),
            Opt::LongWithArg(opt, _) => format!("--{}", opt),
            Opt::Free(_) => "free".to_owned()
        }
    }
}

impl Default for ParsingStyle {
    /// Returns the default parsing style, `AllOptions`.
    fn default() -> ParsingStyle {
        ParsingStyle::AllOptions
    }
}

/// Parses arguments from the command line.
///
/// The first argument (the program name) should be omitted.
pub fn parse_args<T: Options>(args: &[String], style: ParsingStyle) -> Result<T, Error> {
    T::parse_args(args, style)
}

/// Parses arguments from the command line using the default
/// [parsing style](enum.ParsingStyle.html).
///
/// The first argument (the program name) should be omitted.
pub fn parse_args_default<T: Options>(args: &[String]) -> Result<T, Error> {
    T::parse_args_default(args)
}

/// Parses arguments from the environment.
///
/// If an error is encountered, the error is printed to `stderr` and the
/// process will exit with status code `2`.
///
/// If the user supplies a help option, option usage will be printed to
/// `stdout` and the process will exit with status code `0`.
///
/// Otherwise, the parsed options are returned.
///
/// # Panics
///
/// If any argument to the process is not valid unicode.
pub fn parse_args_or_exit<T: Options>(style: ParsingStyle) -> T {
    T::parse_args_or_exit(style)
}

/// Parses arguments from the environment, using the default
/// [parsing style](enum.ParsingStyle.html).
///
/// If an error is encountered, the error is printed to `stderr` and the
/// process will exit with status code `2`.
///
/// If the user supplies a help option, option usage will be printed to
/// `stdout` and the process will exit with status code `0`.
///
/// Otherwise, the parsed options are returned.
///
/// # Panics
///
/// If any argument to the process is not valid unicode.
pub fn parse_args_default_or_exit<T: Options>() -> T {
    T::parse_args_default_or_exit()
}

#[cfg(test)]
mod test {
    use super::{Opt, Parser, ParsingStyle};
    use assert_matches::assert_matches;

    #[test]
    fn test_parser() {
        let args = &["-a", "b", "-cde", "arg", "-xfoo", "--long", "--opt=val",
            "--", "y", "-z"];

        let mut p = Parser::new(args, ParsingStyle::AllOptions);

        assert_matches!(p.next_opt(), Some(Opt::Short('a')));
        assert_matches!(p.next_opt(), Some(Opt::Free("b")));
        assert_matches!(p.next_opt(), Some(Opt::Short('c')));
        assert_matches!(p.next_opt(), Some(Opt::Short('d')));
        assert_matches!(p.next_opt(), Some(Opt::Short('e')));
        assert_matches!(p.next_arg(), Some("arg"));
        assert_matches!(p.next_opt(), Some(Opt::Short('x')));
        assert_matches!(p.next_arg(), Some("foo"));
        assert_matches!(p.next_opt(), Some(Opt::Long("long")));
        assert_matches!(p.next_opt(), Some(Opt::LongWithArg("opt", "val")));
        assert_matches!(p.next_opt(), Some(Opt::Free("y")));
        assert_matches!(p.next_opt(), Some(Opt::Free("-z")));
        assert_matches!(p.next_opt(), None);
    }

    #[test]
    fn test_parsing_style() {
        let args = &["-a", "b", "-c", "--d"];

        let mut p = Parser::new(args, ParsingStyle::AllOptions);

        assert_matches!(p.next_opt(), Some(Opt::Short('a')));
        assert_matches!(p.next_opt(), Some(Opt::Free("b")));
        assert_matches!(p.next_opt(), Some(Opt::Short('c')));
        assert_matches!(p.next_opt(), Some(Opt::Long("d")));
        assert_matches!(p.next_opt(), None);

        let mut p = Parser::new(args, ParsingStyle::StopAtFirstFree);

        assert_matches!(p.next_opt(), Some(Opt::Short('a')));
        assert_matches!(p.next_opt(), Some(Opt::Free("b")));
        assert_matches!(p.next_opt(), Some(Opt::Free("-c")));
        assert_matches!(p.next_opt(), Some(Opt::Free("--d")));
        assert_matches!(p.next_opt(), None);
    }
}
