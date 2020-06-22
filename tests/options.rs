use std::str::FromStr;

use assert_matches::assert_matches;

use gumdrop::Options;

const EMPTY: &'static [&'static str] = &[];

#[derive(Debug, Options)]
struct NoOpts { }

macro_rules! is_err {
    ( $e:expr , |$ident:ident| $expr:expr ) => {
        let $ident = $e.map(|_| ()).unwrap_err().to_string();
        assert!($expr,
            "error {:?} does not match `{}`", $ident, stringify!($expr));
    };
    ( $e:expr , $str:expr ) => {
        assert_eq!($e.map(|_| ()).unwrap_err().to_string(), $str)
    };
}

#[test]
fn test_hygiene() {
    // Define these aliases in local scope to ensure that generated code
    // is using absolute paths, i.e. `::std::result::Result`
    #[allow(dead_code)] struct AsRef;
    #[allow(dead_code)] struct Default;
    #[allow(dead_code)] struct FromStr;
    #[allow(dead_code)] struct Option;
    #[allow(dead_code)] struct Some;
    #[allow(dead_code)] struct None;
    #[allow(dead_code)] struct Options;
    #[allow(dead_code)] struct Result;
    #[allow(dead_code)] struct Ok;
    #[allow(dead_code)] struct Err;
    #[allow(dead_code)] struct String;
    #[allow(dead_code)] struct ToString;
    #[allow(dead_code)] struct Vec;

    #[derive(Options)]
    struct Opts {
        a: i32,
        b: ::std::string::String,
        c: ::std::option::Option<::std::string::String>,
        d: ::std::option::Option<i32>,
        e: ::std::vec::Vec<i32>,
        f: ::std::vec::Vec<::std::string::String>,
        g: ::std::option::Option<(i32, i32)>,

        #[options(command)]
        cmd: ::std::option::Option<Cmd>,
    }

    #[derive(Options)]
    enum Cmd {
        Foo(FooOpts),
        Bar(BarOpts),
    }

    #[derive(Options)]
    struct FooOpts {
        #[options(free)]
        free: ::std::vec::Vec<::std::string::String>,
        a: i32,
    }

    #[derive(Options)]
    struct BarOpts {
        #[options(free)]
        first: ::std::option::Option<::std::string::String>,
        #[options(free)]
        rest: ::std::vec::Vec<::std::string::String>,
        a: i32,
    }

    // This is basically just a compile-pass test, so whatever.
}

#[test]
fn test_command() {
    #[derive(Options)]
    struct Opts {
        help: bool,

        #[options(command)]
        command: Option<Command>,
    }

    #[derive(Debug, Options)]
    enum Command {
        Foo(FooOpts),
        Bar(BarOpts),
        #[options(name = "bzzz")]
        Baz(NoOpts),
        FooBar(NoOpts),
        FooXYZ(NoOpts),
    }

    #[derive(Debug, Options)]
    struct FooOpts {
        foo: Option<String>,
    }

    #[derive(Debug, Options)]
    struct BarOpts {
        #[options(free)]
        free: Vec<String>,
    }

    let opts = Opts::parse_args_default(EMPTY).unwrap();
    assert_eq!(opts.command.is_none(), true);

    let opts = Opts::parse_args_default(&["-h"]).unwrap();
    assert_eq!(opts.help, true);
    assert_eq!(opts.command.is_none(), true);

    let opts = Opts::parse_args_default(&["-h", "foo", "--foo", "x"]).unwrap();
    assert_eq!(opts.help, true);
    let cmd = opts.command.unwrap();
    assert_matches!(cmd, Command::Foo(FooOpts{foo: Some(ref foo)}) if foo == "x");

    let opts = Opts::parse_args_default(&["--", "foo"]).unwrap();
    assert_eq!(opts.help, false);
    let cmd = opts.command.unwrap();
    assert_matches!(cmd, Command::Foo(_));

    let opts = Opts::parse_args_default(&["bar", "free"]).unwrap();
    let cmd = opts.command.unwrap();
    assert_matches!(cmd, Command::Bar(ref bar) if bar.free == ["free"]);

    let opts = Opts::parse_args_default(&["bzzz"]).unwrap();
    let cmd = opts.command.unwrap();
    assert_matches!(cmd, Command::Baz(_));

    let opts = Opts::parse_args_default(&["foo-bar"]).unwrap();
    let cmd = opts.command.unwrap();
    assert_matches!(cmd, Command::FooBar(_));

    let opts = Opts::parse_args_default(&["foo-x-y-z"]).unwrap();
    let cmd = opts.command.unwrap();
    assert_matches!(cmd, Command::FooXYZ(_));

    is_err!(Opts::parse_args_default(&["foo", "-h"]),
        "unrecognized option `-h`");
    is_err!(Opts::parse_args_default(&["baz"]),
        "unrecognized command `baz`");
}

#[test]
fn test_nested_command() {
    #[derive(Debug, Options)]
    struct Main {
        #[options(help = "main help")]
        help: bool,

        #[options(command)]
        command: Option<Command>,
    }

    #[derive(Debug, Options)]
    enum Command {
        #[options(help = "alpha help")]
        Alpha(Alpha),
        #[options(help = "bravo help")]
        Bravo(Bravo),
    }

    #[derive(Debug, Options)]
    struct Alpha {
        #[options(help = "alpha command help")]
        help: bool,

        #[options(command)]
        command: Option<AlphaCommand>,
    }

    #[derive(Debug, Options)]
    struct Bravo {
        #[options(help = "bravo command help")]
        help: bool,

        #[options(help = "bravo option help")]
        option: u32,
    }

    #[derive(Debug, Options)]
    enum AlphaCommand {
        #[options(help = "alpha foo help")]
        Foo(Foo),
        #[options(help = "alpha bar help")]
        Bar(Bar),
    }

    #[derive(Debug, Options)]
    struct Foo {
        #[options(help = "alpha foo command help")]
        help: bool,

        #[options(help = "alpha foo beep help")]
        beep: u32,
    }

    #[derive(Debug, Options)]
    struct Bar {
        #[options(help = "alpha bar command help")]
        help: bool,

        #[options(help = "alpha bar boop help")]
        boop: u32,
    }

    let opts = Main::parse_args_default(&["-h"]).unwrap();
    assert_eq!(opts.self_usage(), Main::usage());

    let opts = Main::parse_args_default(&["-h", "alpha"]).unwrap();
    assert_eq!(opts.self_usage(), Alpha::usage());

    let opts = Main::parse_args_default(&["-h", "bravo"]).unwrap();
    assert_eq!(opts.self_usage(), Bravo::usage());

    let opts = Main::parse_args_default(&["-h", "alpha", "foo"]).unwrap();
    assert_eq!(opts.self_usage(), Foo::usage());

    let opts = Main::parse_args_default(&["-h", "alpha", "bar"]).unwrap();
    assert_eq!(opts.self_usage(), Bar::usage());
}

#[test]
fn test_command_name() {
    #[derive(Options)]
    struct Opts {
        help: bool,

        #[options(command)]
        command: Option<Command>,
    }

    #[derive(Debug, Options)]
    enum Command {
        Foo(NoOpts),
        Bar(NoOpts),
        #[options(name = "bzzz")]
        Baz(NoOpts),
        BoopyDoop(NoOpts),
    }


    let opts = Opts::parse_args_default(&["foo"]).unwrap();
    assert_matches!(opts.command_name(), Some("foo"));

    let opts = Opts::parse_args_default(&["bar"]).unwrap();
    assert_matches!(opts.command_name(), Some("bar"));

    let opts = Opts::parse_args_default(&["bzzz"]).unwrap();
    assert_matches!(opts.command_name(), Some("bzzz"));

    let opts = Opts::parse_args_default(&["boopy-doop"]).unwrap();
    assert_matches!(opts.command_name(), Some("boopy-doop"));
}

#[test]
fn test_command_usage() {
    #[derive(Options)]
    struct Opts {
        #[options(help = "help me!")]
        help: bool,

        #[options(command)]
        command: Option<Command>,
    }

    #[derive(Options)]
    enum Command {
        #[options(help = "foo help")]
        Foo(NoOpts),
        #[options(help = "bar help")]
        Bar(NoOpts),
        #[options(help = "baz help")]
        #[options(name = "bzzz")]
        Baz(NoOpts),
    }

    assert_eq!(Command::usage(), &"
  foo   foo help
  bar   bar help
  bzzz  baz help"
        // Skip leading newline
        [1..]);

    assert_eq!(Command::command_list(), Some(Command::usage()));
    assert_eq!(Opts::command_list(), Some(Command::usage()));
}

#[test]
fn test_opt_bool() {
    #[derive(Options)]
    struct Opts {
        switch: bool,
    }

    let opts = Opts::parse_args_default(&["--switch"]).unwrap();
    assert_eq!(opts.switch, true);

    let opts = Opts::parse_args_default(&["-s"]).unwrap();
    assert_eq!(opts.switch, true);

    is_err!(Opts::parse_args_default(&["--switch=x"]),
        "option `--switch` does not accept an argument");
}

#[test]
fn test_opt_string() {
    #[derive(Options)]
    struct Opts {
        foo: String,
    }

    let opts = Opts::parse_args_default(&["--foo", "value"]).unwrap();
    assert_eq!(opts.foo, "value");

    let opts = Opts::parse_args_default(&["-f", "value"]).unwrap();
    assert_eq!(opts.foo, "value");

    let opts = Opts::parse_args_default(&["-fvalue"]).unwrap();
    assert_eq!(opts.foo, "value");
}

#[test]
fn test_opt_int() {
    #[derive(Options)]
    struct Opts {
        number: i32,
    }

    let opts = Opts::parse_args_default(&["--number", "123"]).unwrap();
    assert_eq!(opts.number, 123);

    let opts = Opts::parse_args_default(&["-n", "123"]).unwrap();
    assert_eq!(opts.number, 123);

    let opts = Opts::parse_args_default(&["-n123"]).unwrap();
    assert_eq!(opts.number, 123);

    is_err!(Opts::parse_args_default(&["-nfail"]),
        |e| e.starts_with("invalid argument to option `-n`: "));
    is_err!(Opts::parse_args_default(&["--number", "fail"]),
        |e| e.starts_with("invalid argument to option `--number`: "));
    is_err!(Opts::parse_args_default(&["--number=fail"]),
        |e| e.starts_with("invalid argument to option `--number`: "));
}

#[test]
fn test_opt_tuple() {
    #[derive(Options)]
    struct Opts {
        alpha: (i32, i32),
        bravo: Option<(i32, i32, i32)>,
        charlie: Vec<(i32, i32, i32, i32)>,
        #[options(free)]
        free: Vec<String>,
    }

    let opts = Opts::parse_args_default(&[
        "--alpha", "1", "2",
        "--bravo", "11", "12", "13",
        "--charlie", "21", "22", "23", "24",
        "--charlie", "31", "32", "33", "34",
        "free",
    ]).unwrap();

    assert_eq!(opts.alpha, (1, 2));
    assert_eq!(opts.bravo, Some((11, 12, 13)));
    assert_eq!(opts.charlie, vec![
        (21, 22, 23, 24),
        (31, 32, 33, 34),
    ]);
    assert_eq!(opts.free, vec!["free".to_owned()]);
}

#[test]
fn test_opt_tuple_error() {
    #[derive(Options)]
    struct Opts {
        foo: Option<(i32, i32)>,
    }

    is_err!(Opts::parse_args_default(&["--foo"]),
        "insufficient arguments to option `--foo`: expected 2; found 0");
    is_err!(Opts::parse_args_default(&["--foo=0", "1"]),
        "option `--foo` expects 2 arguments; found 1");
    is_err!(Opts::parse_args_default(&["--foo", "0"]),
        "insufficient arguments to option `--foo`: expected 2; found 1");
}

#[test]
fn test_opt_push() {
    #[derive(Options)]
    struct Opts {
        thing: Vec<String>,
    }

    let opts = Opts::parse_args_default(EMPTY).unwrap();
    assert!(opts.thing.is_empty());

    let opts = Opts::parse_args_default(
        &["-t", "a", "-tb", "--thing=c", "--thing", "d"]).unwrap();
    assert_eq!(opts.thing, ["a", "b", "c", "d"]);
}

#[test]
fn test_opt_count() {
    #[derive(Options)]
    struct Opts {
        #[options(count)]
        number: i32,
    }

    let opts = Opts::parse_args_default(EMPTY).unwrap();
    assert_eq!(opts.number, 0);

    let opts = Opts::parse_args_default(&["--number"]).unwrap();
    assert_eq!(opts.number, 1);

    let opts = Opts::parse_args_default(&["-nnn"]).unwrap();
    assert_eq!(opts.number, 3);
}

#[test]
fn test_opt_long() {
    #[derive(Options)]
    struct Opts {
        #[options(long = "thing", no_short)]
        foo: bool,
    }

    let opts = Opts::parse_args_default(&["--thing"]).unwrap();
    assert_eq!(opts.foo, true);

    is_err!(Opts::parse_args_default(&["-f"]),
        "unrecognized option `-f`");
    is_err!(Opts::parse_args_default(&["--foo"]),
        "unrecognized option `--foo`");
}

#[test]
fn test_opt_short() {
    #[derive(Options)]
    struct Opts {
        #[options(short = "x", no_long)]
        foo: bool,
    }

    let opts = Opts::parse_args_default(&["-x"]).unwrap();
    assert_eq!(opts.foo, true);

    is_err!(Opts::parse_args_default(&["-f"]),
        "unrecognized option `-f`");
    is_err!(Opts::parse_args_default(&["--foo"]),
        "unrecognized option `--foo`");
}

#[test]
fn test_opt_short_override() {
    // Ensures that the generated code sees the manual assignment of short
    // option for `option_1` before generating a short option for `option_0`.
    // Thus, giving `option_0` an automatic short option of `O`,
    // rather than causing a collision.
    #[derive(Options)]
    struct Opts {
        #[options(no_long)]
        option_0: bool,
        #[options(short = "o", no_long)]
        option_1: bool,
    }

    let opts = Opts::parse_args_default(&["-o"]).unwrap();
    assert_eq!(opts.option_0, false);
    assert_eq!(opts.option_1, true);

    let opts = Opts::parse_args_default(&["-O"]).unwrap();
    assert_eq!(opts.option_0, true);
    assert_eq!(opts.option_1, false);
}

#[test]
fn test_opt_free() {
    #[derive(Options)]
    struct Opts {
        #[options(free)]
        free: Vec<String>,
    }

    let opts = Opts::parse_args_default(&["a", "b", "c"]).unwrap();
    assert_eq!(opts.free, ["a", "b", "c"]);
}

#[test]
fn test_opt_no_free() {
    #[derive(Options)]
    struct Opts {
    }

    assert!(Opts::parse_args_default(EMPTY).is_ok());
    is_err!(Opts::parse_args_default(&["a"]),
        "unexpected free argument `a`");
}

#[test]
fn test_typed_free() {
    #[derive(Options)]
    struct Opts {
        #[options(free)]
        free: Vec<i32>,
    }

    let opts = Opts::parse_args_default(&["1", "2", "3"]).unwrap();
    assert_eq!(opts.free, [1, 2, 3]);
}

#[test]
fn test_multi_free() {
    #[derive(Options)]
    struct Opts {
        #[options(free, help = "alpha help")]
        alpha: u32,
        #[options(free, help = "bravo help")]
        bravo: Option<String>,
        #[options(free, help = "charlie help")]
        charlie: Option<u32>,
    }

    let opts = Opts::parse_args_default(EMPTY).unwrap();

    assert_eq!(opts.alpha, 0);
    assert_eq!(opts.bravo, None);
    assert_eq!(opts.charlie, None);

    let opts = Opts::parse_args_default(&["1"]).unwrap();

    assert_eq!(opts.alpha, 1);
    assert_eq!(opts.bravo, None);
    assert_eq!(opts.charlie, None);

    let opts = Opts::parse_args_default(&["1", "two", "3"]).unwrap();

    assert_eq!(opts.alpha, 1);
    assert_eq!(opts.bravo, Some("two".to_owned()));
    assert_eq!(opts.charlie, Some(3));

    is_err!(Opts::parse_args_default(&["1", "two", "3", "4"]),
        "unexpected free argument `4`");

    assert_eq!(Opts::usage(), &"
Positional arguments:
  alpha    alpha help
  bravo    bravo help
  charlie  charlie help"
        // Skip leading newline
        [1..]);

    #[derive(Options)]
    struct ManyOpts {
        #[options(free, help = "alpha help")]
        alpha: u32,
        #[options(free, help = "bravo help")]
        bravo: Option<String>,
        #[options(free, help = "charlie help")]
        charlie: Option<u32>,
        #[options(free)]
        rest: Vec<String>,
    }

    let opts = ManyOpts::parse_args_default(EMPTY).unwrap();

    assert_eq!(opts.alpha, 0);
    assert_eq!(opts.bravo, None);
    assert_eq!(opts.charlie, None);
    assert_eq!(opts.rest, Vec::<String>::new());

    let opts = ManyOpts::parse_args_default(&["1", "two", "3", "4", "five", "VI"]).unwrap();

    assert_eq!(opts.alpha, 1);
    assert_eq!(opts.bravo, Some("two".to_owned()));
    assert_eq!(opts.charlie, Some(3));
    assert_eq!(opts.rest, vec!["4".to_owned(), "five".to_owned(), "VI".to_owned()]);
}

#[test]
fn test_usage() {
    #[derive(Options)]
    struct Opts {
        #[options(help = "alpha help")]
        alpha: bool,
        #[options(no_short, help = "bravo help")]
        bravo: String,
        #[options(no_long, help = "charlie help")]
        charlie: bool,
        #[options(help = "delta help", meta = "X")]
        delta: i32,
        #[options(help = "echo help", meta = "Y")]
        echo: Vec<String>,
        #[options(help = "foxtrot help", meta = "Z", default = "99")]
        foxtrot: u32,
        #[options(no_short, help = "long option help")]
        very_very_long_option_with_very_very_long_name: bool,
    }

    assert_eq!(Opts::usage(), &"
Optional arguments:
  -a, --alpha      alpha help
  --bravo BRAVO    bravo help
  -c               charlie help
  -d, --delta X    delta help
  -e, --echo Y     echo help
  -f, --foxtrot Z  foxtrot help (default: 99)
  --very-very-long-option-with-very-very-long-name
                   long option help"
        // Skip leading newline
        [1..]);

    #[derive(Options)]
    struct TupleOpts {
        #[options(help = "alpha help")]
        alpha: (),
        #[options(help = "bravo help")]
        bravo: (i32,),
        #[options(help = "charlie help")]
        charlie: (i32, i32),
        #[options(help = "delta help")]
        delta: (i32, i32, i32),
        #[options(help = "echo help")]
        echo: (i32, i32, i32, i32),
    }

    assert_eq!(TupleOpts::usage(), &"
Optional arguments:
  -a, --alpha        alpha help
  -b, --bravo BRAVO  bravo help
  -c, --charlie CHARLIE VALUE
                     charlie help
  -d, --delta DELTA VALUE0 VALUE1
                     delta help
  -e, --echo ECHO VALUE0 VALUE1 VALUE2
                     echo help"
        // Skip leading newline
        [1..]);

    #[derive(Options)]
    struct FreeOpts {
        #[options(free, help = "a help")]
        a: u32,
        #[options(free, help = "b help")]
        b: u32,
        #[options(free, help = "c help")]
        c: u32,

        #[options(help = "option help")]
        option: bool,
    }

    assert_eq!(FreeOpts::usage(), &"
Positional arguments:
  a             a help
  b             b help
  c             c help

Optional arguments:
  -o, --option  option help"
        // Skip leading newline
        [1..]);
}

#[test]
fn test_help_flag() {
    #[derive(Options)]
    struct Opts {
        help: bool,
    }

    let opts = Opts::parse_args_default(EMPTY).unwrap();
    assert_eq!(opts.help_requested(), false);

    let opts = Opts::parse_args_default(&["--help"]).unwrap();
    assert_eq!(opts.help_requested(), true);
}

#[test]
fn test_no_help_flag() {
    #[derive(Options)]
    struct Opts {
        #[options(no_help_flag)]
        help: bool,
    }

    let opts = Opts::parse_args_default(&["--help"]).unwrap();
    assert_eq!(opts.help_requested(), false);
}

#[test]
fn test_many_help_flags() {
    #[derive(Options)]
    struct Opts {
        #[options(help_flag)]
        help: bool,
        #[options(help_flag)]
        halp: bool,
        #[options(help_flag)]
        help_please: bool,
    }

    let opts = Opts::parse_args_default(EMPTY).unwrap();
    assert_eq!(opts.help_requested(), false);

    let opts = Opts::parse_args_default(&["--help"]).unwrap();
    assert_eq!(opts.help_requested(), true);

    let opts = Opts::parse_args_default(&["--halp"]).unwrap();
    assert_eq!(opts.help_requested(), true);

    let opts = Opts::parse_args_default(&["--help-please"]).unwrap();
    assert_eq!(opts.help_requested(), true);
}

#[test]
fn test_help_flag_command() {
    #[derive(Options)]
    struct Opts {
        help: bool,

        #[options(command)]
        cmd: Option<Cmd>,
    }

    #[derive(Options)]
    struct Opts2 {
        #[options(command)]
        cmd: Option<Cmd>,
    }

    #[derive(Options)]
    struct Opts3 {
        help: bool,
        #[options(help_flag)]
        help2: bool,

        #[options(command)]
        cmd: Option<Cmd>,
    }

    #[derive(Options)]
    enum Cmd {
        Foo(CmdOpts),
        Bar(CmdOpts),
        Baz(CmdOpts),
    }

    #[derive(Options)]
    struct CmdOpts {
        help: bool,
    }

    let opts = Opts::parse_args_default(EMPTY).unwrap();
    assert_eq!(opts.help_requested(), false);

    let opts = Opts::parse_args_default(&["-h"]).unwrap();
    assert_eq!(opts.help_requested(), true);

    let opts = Opts::parse_args_default(&["foo", "-h"]).unwrap();
    assert_eq!(opts.help_requested(), true);

    let opts = Opts::parse_args_default(&["bar", "-h"]).unwrap();
    assert_eq!(opts.help_requested(), true);

    let opts = Opts::parse_args_default(&["baz", "-h"]).unwrap();
    assert_eq!(opts.help_requested(), true);

    let opts = Opts2::parse_args_default(EMPTY).unwrap();
    assert_eq!(opts.help_requested(), false);

    let opts = Opts3::parse_args_default(EMPTY).unwrap();
    assert_eq!(opts.help_requested(), false);
}

#[test]
fn test_type_attrs() {
    #[derive(Options)]
    #[options(no_help_flag, no_short, no_long)]
    struct Opts {
        #[options(long = "help")]
        help: bool,
        #[options(long = "foo")]
        foo: bool,
        #[options(short = "b")]
        bar: bool,
    }

    is_err!(Opts::parse_args_default(&["-f"]),
        "unrecognized option `-f`");
    is_err!(Opts::parse_args_default(&["--bar"]),
        "unrecognized option `--bar`");
    is_err!(Opts::parse_args_default(&["-h"]),
        "unrecognized option `-h`");

    let opts = Opts::parse_args_default(&["--help"]).unwrap();
    assert_eq!(opts.help, true);
    assert_eq!(opts.help_requested(), false);

    let opts = Opts::parse_args_default(&["--foo"]).unwrap();
    assert_eq!(opts.foo, true);

    let opts = Opts::parse_args_default(&["-b"]).unwrap();
    assert_eq!(opts.bar, true);

    #[derive(Options)]
    #[options(no_short)]
    struct Opts2 {
        foo: bool,
        #[options(short = "b")]
        bar: bool,
    }

    is_err!(Opts2::parse_args_default(&["-f"]),
        "unrecognized option `-f`");

    let opts = Opts2::parse_args_default(&["--foo", "-b"]).unwrap();
    assert_eq!(opts.foo, true);
    assert_eq!(opts.bar, true);

    let opts = Opts2::parse_args_default(&["--bar"]).unwrap();
    assert_eq!(opts.bar, true);

    #[derive(Options)]
    #[options(no_long)]
    struct Opts3 {
        foo: bool,
        #[options(long = "bar")]
        bar: bool,
    }

    is_err!(Opts3::parse_args_default(&["--foo"]),
        "unrecognized option `--foo`");

    let opts = Opts3::parse_args_default(&["--bar"]).unwrap();
    assert_eq!(opts.bar, true);

    let opts = Opts3::parse_args_default(&["-f", "-b"]).unwrap();
    assert_eq!(opts.foo, true);
    assert_eq!(opts.bar, true);

    #[derive(Options)]
    #[options(no_help_flag)]
    struct Opts4 {
        #[options(help_flag)]
        help: bool,
    }

    let opts = Opts4::parse_args_default(&["-h"]).unwrap();
    assert_eq!(opts.help, true);
    assert_eq!(opts.help_requested(), true);

    #[derive(Options)]
    #[options(required)]
    struct Opts5 {
        #[options(no_long)]
        foo: i32,
        #[options(not_required)]
        bar: i32,
    }

    is_err!(Opts5::parse_args_default(EMPTY),
        "missing required option `-f`");

    let opts = Opts5::parse_args_default(&["-f", "1"]).unwrap();
    assert_eq!(opts.foo, 1);
    assert_eq!(opts.bar, 0);

    let opts = Opts5::parse_args_default(&["-f", "1", "--bar", "2"]).unwrap();
    assert_eq!(opts.foo, 1);
    assert_eq!(opts.bar, 2);
}

#[test]
fn test_required() {
    #[derive(Options)]
    struct Opts {
        #[options(required)]
        foo: i32,
        optional: i32,
    }

    #[derive(Options)]
    struct Opts2 {
        #[options(command, required)]
        command: Option<Cmd>,
        optional: i32,
    }

    #[derive(Options)]
    enum Cmd {
        Foo(NoOpts),
    }

    #[derive(Options)]
    struct Opts3 {
        #[options(free, required)]
        bar: i32,
        optional: i32,
    }

    is_err!(Opts::parse_args_default(EMPTY),
        "missing required option `--foo`");
    is_err!(Opts2::parse_args_default(EMPTY),
        "missing required command");
    is_err!(Opts3::parse_args_default(EMPTY),
        "missing required free argument");

    let opts = Opts::parse_args_default(&["-f", "1"]).unwrap();
    assert_eq!(opts.foo, 1);
    let opts = Opts::parse_args_default(&["-f1"]).unwrap();
    assert_eq!(opts.foo, 1);
    let opts = Opts::parse_args_default(&["--foo", "1"]).unwrap();
    assert_eq!(opts.foo, 1);
    let opts = Opts::parse_args_default(&["--foo=1"]).unwrap();
    assert_eq!(opts.foo, 1);

    let opts = Opts2::parse_args_default(&["foo"]).unwrap();
    assert!(opts.command.is_some());

    let opts = Opts3::parse_args_default(&["1"]).unwrap();
    assert_eq!(opts.bar, 1);
}

#[test]
fn test_required_help() {
    #[derive(Options)]
    struct Opts {
        #[options(required)]
        thing: Option<String>,
        help: bool,
    }

    #[derive(Options)]
    struct Opts2 {
        #[options(required)]
        thing: Option<String>,
        help: bool,
        #[options(help_flag)]
        secondary_help: bool,
    }

    let opts = Opts::parse_args_default(&["-h"]).unwrap();
    assert_eq!(opts.help, true);

    let opts = Opts2::parse_args_default(&["--secondary-help"]).unwrap();
    assert_eq!(opts.secondary_help, true);
}

#[test]
fn test_parse() {
    #[derive(Options)]
    struct Opts {
        #[options(help = "foo", parse(from_str = "parse_foo"))]
        foo: Option<Foo>,
        #[options(help = "bar", parse(try_from_str = "parse_bar"))]
        bar: Option<Bar>,
        #[options(help = "baz", parse(from_str))]
        baz: Option<Baz>,
        #[options(help = "quux", parse(try_from_str))]
        quux: Option<Quux>,
    }

    #[derive(Debug)]
    struct Foo(String);
    #[derive(Debug)]
    struct Bar(u32);
    #[derive(Debug)]
    struct Baz(String);
    #[derive(Debug)]
    struct Quux(u32);

    fn parse_foo(s: &str) -> Foo { Foo(s.to_owned()) }
    fn parse_bar(s: &str) -> Result<Bar, <u32 as FromStr>::Err> { s.parse().map(Bar) }

    impl<'a> From<&'a str> for Baz {
        fn from(s: &str) -> Baz {
            Baz(s.to_owned())
        }
    }

    impl FromStr for Quux {
        type Err = <u32 as FromStr>::Err;

        fn from_str(s: &str) -> Result<Quux, Self::Err> {
            s.parse().map(Quux)
        }
    }

    let opts = Opts::parse_args_default(&[
        "-ffoo", "--bar=123", "--baz", "sup", "-q", "456"]).unwrap();
    assert_matches!(opts.foo, Some(Foo(ref s)) if s == "foo");
    assert_matches!(opts.bar, Some(Bar(123)));
    assert_matches!(opts.baz, Some(Baz(ref s)) if s == "sup");
    assert_matches!(opts.quux, Some(Quux(456)));

    is_err!(Opts::parse_args_default(&["--bar", "xyz"]),
        |e| e.starts_with("invalid argument to option `--bar`: "));
    is_err!(Opts::parse_args_default(&["--quux", "xyz"]),
        |e| e.starts_with("invalid argument to option `--quux`: "));
}

#[test]
fn test_default() {
    #[derive(Options)]
    struct Opts {
        foo: u32,
        #[options(default = "123")]
        bar: u32,
        #[options(default = "456")]
        baz: Baz,
        #[options(count, default = "789")]
        count: u32,
    }

    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    struct Baz(u32);

    impl FromStr for Baz {
        type Err = <u32 as FromStr>::Err;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            s.parse().map(Baz)
        }
    }

    let opts = Opts::parse_args_default(EMPTY).unwrap();
    assert_eq!(opts.foo, 0);
    assert_eq!(opts.bar, 123);
    assert_eq!(opts.baz, Baz(456));
    assert_eq!(opts.count, 789);

    let opts = Opts::parse_args_default(&["-b99", "--baz=4387", "-c", "-f1"]).unwrap();
    assert_eq!(opts.foo, 1);
    assert_eq!(opts.bar, 99);
    assert_eq!(opts.baz, Baz(4387));
    assert_eq!(opts.count, 790);
}

#[test]
fn test_failed_default() {
    #[derive(Options)]
    struct Opts {
        #[options(default = "lolwut")]
        foo: u32,
    }

    is_err!(Opts::parse_args_default(EMPTY),
        |e| e.starts_with(r#"invalid default value for `foo` ("lolwut"): "#));
}

#[test]
fn test_default_parse() {
    #[derive(Options)]
    struct Opts {
        #[options(default = "1", parse(try_from_str = "parse_foo"))]
        foo: Foo,
    }

    #[derive(Debug, Eq, PartialEq)]
    struct Foo(u32);

    fn parse_foo(s: &str) -> Result<Foo, <u32 as FromStr>::Err> { s.parse().map(Foo) }

    let opts = Opts::parse_args_default(EMPTY).unwrap();
    assert_eq!(opts.foo, Foo(1));
}

#[test]
fn test_multi() {
    use std::collections::VecDeque;

    #[derive(Options)]
    struct Opts {
        #[options(multi = "push_back")]
        foo: VecDeque<String>,
    }

    #[derive(Options)]
    struct Opts2 {
        #[options(multi = "push_back")]
        foo: VecDeque<(i32, i32)>,
    }

    #[derive(Options)]
    struct Opts3 {
        #[options(free, multi = "push_front")]
        free: VecDeque<i32>,
    }

    let opts = Opts::parse_args_default(&["-f", "foo", "-f", "bar"]).unwrap();
    assert_eq!(opts.foo, ["foo", "bar"]);

    let opts = Opts2::parse_args_default(&["-f", "1", "2", "-f", "3", "4"]).unwrap();
    assert_eq!(opts.foo, [(1, 2), (3, 4)]);

    let opts = Opts3::parse_args_default(&["1", "2", "3"]).unwrap();
    assert_eq!(opts.free, [3, 2, 1]);
}

#[test]
fn test_no_multi() {
    #[derive(Options)]
    struct Opts {
        #[options(no_multi, parse(from_str = "comma_list"))]
        list_things: Vec<String>,
    }

    #[derive(Options)]
    #[options(no_multi)]
    struct Opts2 {
        #[options(parse(from_str = "comma_list"))]
        list_things: Vec<String>,
    }

    #[derive(Options)]
    struct Opts3 {
        #[options(free, no_multi, parse(from_str = "comma_list"))]
        list_things: Vec<String>,
    }

    fn comma_list(s: &str) -> Vec<String> {
        s.split(',').map(|s| s.to_string()).collect()
    }

    let opts = Opts::parse_args_default(&["-l", "foo,bar,baz"]).unwrap();
    assert_eq!(opts.list_things, ["foo", "bar", "baz"]);

    let opts = Opts2::parse_args_default(&["-l", "foo,bar,baz"]).unwrap();
    assert_eq!(opts.list_things, ["foo", "bar", "baz"]);

    let opts = Opts3::parse_args_default(&["foo,bar,baz"]).unwrap();
    assert_eq!(opts.list_things, ["foo", "bar", "baz"]);

    is_err!(Opts3::parse_args_default(&["foo,bar,baz", "error"]),
        "unexpected free argument `error`");
}

#[test]
fn test_doc_help() {
    /// type-level help comment
    #[derive(Options)]
    struct Opts {
        /// free help comment
        #[options(free)]
        free: i32,
        /// help comment
        foo: i32,
        /// help comment
        #[options(help = "help attribute")]
        bar: i32,
    }

    #[derive(Options)]
    enum Cmd {
        /// help comment
        Alpha(NoOpts),
        /// help comment
        #[options(help = "help attribute")]
        Bravo(NoOpts),
    }

    assert_eq!(Opts::usage(), &"
type-level help comment

Positional arguments:
  free           free help comment

Optional arguments:
  -f, --foo FOO  help comment
  -b, --bar BAR  help attribute"
        // Skip leading newline
        [1..]);

    assert_eq!(Cmd::usage(), &"
  alpha  help comment
  bravo  help attribute"
        // Skip leading newline
        [1..]);
}

#[test]
fn test_doc_help_multiline() {
    /// type-level help comment
    /// second line of text
    #[derive(Options)]
    struct Opts {
        /// help comment
        foo: i32,
    }

    assert_eq!(Opts::usage(), &"
type-level help comment
second line of text

Optional arguments:
  -f, --foo FOO  help comment"
        // Skip leading newline
        [1..]);
}

#[test]
fn test_failed_parse_free() {
    #[derive(Options)]
    struct Opts {
        #[options(free)]
        foo: u32,
        #[options(free, parse(try_from_str = "parse"))]
        bar: u32,
        #[options(free)]
        baz: Vec<u32>,
    }

    fn parse(s: &str) -> Result<u32, <u32 as FromStr>::Err> {
        s.parse()
    }

    is_err!(Opts::parse_args_default(&["x"]),
        |e| e.starts_with("invalid argument to option `foo`: "));

    is_err!(Opts::parse_args_default(&["0", "x"]),
        |e| e.starts_with("invalid argument to option `bar`: "));

    is_err!(Opts::parse_args_default(&["0", "0", "x"]),
        |e| e.starts_with("invalid argument to option `baz`: "));
}

#[cfg(feature = "default_expr")]
#[test]
fn test_default_expr() {
    #[derive(Options)]
    struct Opts {
        #[options(default_expr = "foo()")]
        foo: u32,
    }

    fn foo() -> u32 { 123 }

    let opts = Opts::parse_args_default(EMPTY).unwrap();
    assert_eq!(opts.foo, foo());
}
