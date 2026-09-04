//! Command-line parsing.
//!
//! Hand-rolled rather than delegated to an argument-parsing crate, so that the
//! `cli` feature stays as dependency-free as the library it wraps.

use std::fmt;

use charcode::Encoding;

pub const USAGE: &str = "\
Usage: charcode [OPTION]... [FILE]...
Convert text from one encoding to another.

Input and output encodings:
  -f, --from-code=NAME     encoding of the input (default UTF-8)
  -t, --to-code=NAME       encoding of the output (default UTF-8)

Error handling (the default is to stop at the first problem):
  -c                       omit malformed input and unmappable characters
      --substitute         substitute instead: U+FFFD for malformed input,
                           a numeric character reference for a character the
                           output encoding cannot represent

Output:
  -o, --output=FILE        write to FILE instead of standard output
  -s, --silent             suppress the summary of omitted characters
      --verbose            report the encodings and byte counts on stderr

Information:
  -l, --list               list the encodings this build supports
      --list-labels        list every label, with the encoding it names
  -h, --help               show this help
  -V, --version            show the version

With no FILE, or when FILE is -, read standard input.  Several files are
converted as one stream, which is what concatenating them would produce.

The exit status is 0 only if every byte converted faithfully.  As with iconv,
-c still exits non-zero when something had to be dropped, so a script can tell
a clean conversion from a lossy one.

A NAME may be any label from the WHATWG Encoding Standard, so \"latin1\",
\"ISO-8859-1\" and \"windows-1252\" all name the same encoding.  A Windows code
page number also works, written with its usual prefix: cp932, windows-1252,
ibm866, x-cp20936.  For compatibility with iconv, a \"//IGNORE\" suffix on
either NAME is accepted and means the same as -c.
";

/// What to do about input that cannot be decoded, or characters the output
/// encoding cannot represent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnError {
    /// Stop and report, which is the default and what iconv does.
    Fail,
    /// Drop it and carry on.
    Omit,
    /// Write U+FFFD, or a numeric character reference, as the standard requires
    /// of web content.
    Substitute,
}

/// What the program was asked to do.
#[derive(Debug)]
pub enum Command {
    Convert(Box<Options>),
    ListEncodings,
    ListLabels,
    Help,
    Version,
}

#[derive(Debug)]
pub struct Options {
    pub from: &'static Encoding,
    pub to: &'static Encoding,
    pub inputs: Vec<Input>,
    pub output: Option<String>,
    pub on_error: OnError,
    pub silent: bool,
    pub verbose: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    Stdin,
    File(String),
}

impl fmt::Display for Input {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Input::Stdin => f.write_str("(standard input)"),
            Input::File(path) => f.write_str(path),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\nTry 'charcode --help' for more information.", self.0)
    }
}

fn error<T>(message: impl Into<String>) -> Result<T, ParseError> {
    Err(ParseError(message.into()))
}

/// Resolves a `-f`/`-t` value, honouring iconv's `//` suffixes.
///
/// Returns the encoding and whether the suffix asked for characters to be
/// dropped.
fn resolve(kind: &str, value: &str) -> Result<(&'static Encoding, bool), ParseError> {
    let (name, suffix) = match value.split_once("//") {
        Some((name, suffix)) => (name, suffix),
        None => (value, ""),
    };
    let ignore = match suffix.to_ascii_uppercase().as_str() {
        "" => false,
        "IGNORE" => true,
        "TRANSLIT" => {
            return error(
                "//TRANSLIT is not supported: charcode does not transliterate. \
                 Use -c to omit characters the output encoding cannot represent.",
            );
        }
        other => return error(format!("unknown suffix '//{other}' in {kind} '{value}'")),
    };
    if let Some(encoding) = Encoding::for_label_no_replacement(name.as_bytes()) {
        return Ok((encoding, ignore));
    }
    if let Some(encoding) = code_page(name)
        && Encoding::for_label_no_replacement(encoding.name().as_bytes()).is_some()
    {
        return Ok((encoding, ignore));
    }
    if Encoding::for_label(name.as_bytes()).is_some() || code_page(name).is_some() {
        return error(format!(
            "{kind} '{name}' names the 'replacement' encoding, which exists only \
             to neutralize labels that are unsafe to support"
        ));
    }
    error(format!("unknown {kind} '{name}'"))
}

/// Resolves a code page written the way iconv and ODBC spell them: `cp932`,
/// `CP-932`, `windows-932`, `x-cp20936`.
///
/// A prefix is required.  A bare number is not a charset name — `-f 932` is far
/// more likely to be a mistake than a request for Shift_JIS — so it stays an
/// error.
///
/// This is a courtesy for scripts written against iconv; the library's label
/// lookup stays exactly what the standard defines, and is tried first.
fn code_page(name: &str) -> Option<&'static Encoding> {
    let lower = name.to_ascii_lowercase();
    let rest = ["x-cp", "cp", "ibm", "windows", "ms", "dos"]
        .iter()
        .find_map(|prefix| lower.strip_prefix(prefix))?;
    let digits = rest.strip_prefix(['-', '_']).unwrap_or(rest);
    Encoding::for_cp(digits.parse().ok()?)
}

/// Parses the arguments after `argv[0]`.
pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Result<Command, ParseError> {
    let mut from = charcode::UTF_8;
    let mut to = charcode::UTF_8;
    let mut inputs = Vec::new();
    let mut output = None;
    let mut on_error = OnError::Fail;
    let mut suffix_ignore = false;
    let mut silent = false;
    let mut verbose = false;
    let mut only_operands = false;

    let mut args = args.into_iter().peekable();
    while let Some(arg) = args.next() {
        if only_operands || arg == "-" || !arg.starts_with('-') {
            inputs.push(if arg == "-" {
                Input::Stdin
            } else {
                Input::File(arg)
            });
            continue;
        }
        if arg == "--" {
            only_operands = true;
            continue;
        }

        if let Some(long) = arg.strip_prefix("--") {
            let (name, inline) = match long.split_once('=') {
                Some((name, value)) => (name, Some(value.to_owned())),
                None => (long, None),
            };
            let value = |args: &mut std::iter::Peekable<_>| match inline.clone() {
                Some(value) => Ok(value),
                None => match Iterator::next(args) {
                    Some(value) => Ok(value),
                    None => error(format!("--{name} needs a value")),
                },
            };
            match name {
                "from-code" => {
                    let (encoding, ignore) = resolve("input encoding", &value(&mut args)?)?;
                    from = encoding;
                    suffix_ignore |= ignore;
                }
                "to-code" => {
                    let (encoding, ignore) = resolve("output encoding", &value(&mut args)?)?;
                    to = encoding;
                    suffix_ignore |= ignore;
                }
                "output" => output = Some(value(&mut args)?),
                "substitute" => on_error = OnError::Substitute,
                "silent" => silent = true,
                "verbose" => verbose = true,
                "list" => return Ok(Command::ListEncodings),
                "list-labels" => return Ok(Command::ListLabels),
                "help" => return Ok(Command::Help),
                "version" => return Ok(Command::Version),
                _ => return error(format!("unknown option '--{name}'")),
            }
            if inline.is_some() && !takes_value(name) {
                return error(format!("option '--{name}' takes no value"));
            }
            continue;
        }

        // A cluster of short options; only the last one may take a value, which
        // can be attached (`-fUTF-8`) or separate (`-f UTF-8`).
        let cluster: Vec<char> = arg[1..].chars().collect();
        for (i, flag) in cluster.iter().enumerate() {
            let rest: String = cluster[i + 1..].iter().collect();
            let value = |args: &mut std::iter::Peekable<_>| {
                if !rest.is_empty() {
                    return Ok(rest.clone());
                }
                match Iterator::next(args) {
                    Some(value) => Ok(value),
                    None => error(format!("-{flag} needs a value")),
                }
            };
            match flag {
                'f' => {
                    let (encoding, ignore) = resolve("input encoding", &value(&mut args)?)?;
                    from = encoding;
                    suffix_ignore |= ignore;
                }
                't' => {
                    let (encoding, ignore) = resolve("output encoding", &value(&mut args)?)?;
                    to = encoding;
                    suffix_ignore |= ignore;
                }
                'o' => output = Some(value(&mut args)?),
                'c' => {
                    on_error = OnError::Omit;
                    continue;
                }
                's' => {
                    silent = true;
                    continue;
                }
                'l' => return Ok(Command::ListEncodings),
                'h' => return Ok(Command::Help),
                'V' => return Ok(Command::Version),
                _ => return error(format!("unknown option '-{flag}'")),
            }
            // Any option reaching here consumed the rest of the cluster.
            break;
        }
    }

    if suffix_ignore && on_error == OnError::Fail {
        on_error = OnError::Omit;
    }
    if inputs.is_empty() {
        inputs.push(Input::Stdin);
    }
    Ok(Command::Convert(Box::new(Options {
        from,
        to,
        inputs,
        output,
        on_error,
        silent,
        verbose,
    })))
}

fn takes_value(long: &str) -> bool {
    matches!(long, "from-code" | "to-code" | "output")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_args(args: &[&str]) -> Result<Command, ParseError> {
        parse(args.iter().map(|s| (*s).to_owned()))
    }

    fn parse_options(args: &[&str]) -> Options {
        match parse_args(args).expect("parses") {
            Command::Convert(options) => *options,
            other => panic!("expected a conversion, got {other:?}"),
        }
    }

    #[test]
    fn defaults_to_utf8_on_stdin() {
        let options = parse_options(&[]);
        assert_eq!(options.from, charcode::UTF_8);
        assert_eq!(options.to, charcode::UTF_8);
        assert_eq!(options.inputs, [Input::Stdin]);
        assert_eq!(options.on_error, OnError::Fail);
        assert!(options.output.is_none());
    }

    #[test]
    fn encodings_can_be_given_in_every_spelling() {
        for args in [
            &["-f", "latin1", "-t", "utf8"][..],
            &["-flatin1", "-tutf8"],
            &["--from-code", "latin1", "--to-code", "utf8"],
            &["--from-code=latin1", "--to-code=utf8"],
        ] {
            let options = parse_options(args);
            assert_eq!(options.from, charcode::WINDOWS_1252, "{args:?}");
            assert_eq!(options.to, charcode::UTF_8, "{args:?}");
        }
    }

    #[test]
    fn short_options_cluster() {
        let options = parse_options(&["-cs", "-fkoi8-r"]);
        assert_eq!(options.on_error, OnError::Omit);
        assert!(options.silent);
        assert_eq!(options.from, charcode::KOI8_R);
        // A value-taking option ends the cluster and takes the rest as its value.
        let options = parse_options(&["-csfkoi8-r"]);
        assert_eq!(options.from, charcode::KOI8_R);
        assert!(options.silent);
    }

    #[test]
    fn files_and_stdin_mix() {
        let options = parse_options(&["a.txt", "-", "b.txt"]);
        assert_eq!(
            options.inputs,
            [
                Input::File("a.txt".into()),
                Input::Stdin,
                Input::File("b.txt".into()),
            ]
        );
        // Everything after -- is a file, even if it looks like an option.
        let options = parse_options(&["--", "-f"]);
        assert_eq!(options.inputs, [Input::File("-f".into())]);
    }

    #[test]
    fn windows_code_pages_are_accepted() {
        for spelling in ["cp932", "CP-932", "cp_932", "windows-932", "x-cp932"] {
            let options = parse_options(&["-f", spelling]);
            assert_eq!(options.from, charcode::SHIFT_JIS, "{spelling}");
        }
        assert_eq!(
            parse_options(&["-f", "cp1252"]).from,
            charcode::WINDOWS_1252
        );
        assert_eq!(parse_options(&["-f", "cp949"]).from, charcode::EUC_KR);
        assert_eq!(parse_options(&["-f", "ibm866"]).from, charcode::IBM866);
        assert_eq!(parse_options(&["-f", "cp65001"]).from, charcode::UTF_8);
        // A label always wins over a number that would mean something else.
        assert_eq!(
            parse_options(&["-f", "latin1"]).from,
            charcode::WINDOWS_1252
        );
        // A bare number is not a charset name.
        for bare in ["932", "1252", "65001", "0"] {
            assert!(parse_args(&["-f", bare]).is_err(), "{bare}");
        }
        // Code pages for charsets this build does not have stay unknown.
        #[cfg(not(feature = "dos"))]
        assert!(parse_args(&["-f", "cp437"]).is_err());
        #[cfg(feature = "dos")]
        assert_eq!(parse_options(&["-f", "cp437"]).from, charcode::IBM437);
        // And the neutralized ones are refused with the same explanation.
        let message = parse_args(&["-f", "cp50225"]).unwrap_err().0;
        assert!(message.contains("replacement"), "{message}");
    }

    #[test]
    fn iconv_suffixes() {
        let options = parse_options(&["-t", "ascii//IGNORE"]);
        assert_eq!(options.to, charcode::WINDOWS_1252);
        assert_eq!(options.on_error, OnError::Omit);
        // An explicit --substitute is not overridden by the suffix.
        let options = parse_options(&["--substitute", "-t", "ascii//IGNORE"]);
        assert_eq!(options.on_error, OnError::Substitute);
        assert!(parse_args(&["-t", "utf-8//TRANSLIT"]).is_err());
        assert!(parse_args(&["-t", "utf-8//NOPE"]).is_err());
    }

    #[test]
    fn bad_input_is_diagnosed() {
        assert!(parse_args(&["-f", "nonsense"]).is_err());
        assert!(parse_args(&["-f"]).is_err());
        assert!(parse_args(&["--from-code"]).is_err());
        assert!(parse_args(&["-x"]).is_err());
        assert!(parse_args(&["--nope"]).is_err());
        assert!(parse_args(&["--silent=yes"]).is_err());
        // The replacement encoding is rejected with an explanation.
        let message = parse_args(&["-f", "hz-gb-2312"]).unwrap_err().0;
        assert!(message.contains("replacement"), "{message}");
    }

    #[test]
    fn informational_commands_win() {
        assert!(matches!(parse_args(&["-l"]), Ok(Command::ListEncodings)));
        assert!(matches!(
            parse_args(&["--list"]),
            Ok(Command::ListEncodings)
        ));
        assert!(matches!(
            parse_args(&["--list-labels"]),
            Ok(Command::ListLabels)
        ));
        assert!(matches!(parse_args(&["-h"]), Ok(Command::Help)));
        assert!(matches!(parse_args(&["-V"]), Ok(Command::Version)));
    }
}
