//! Command-line parsing.
//!
//! Hand-rolled rather than delegated to an argument-parsing crate, so that the
//! `cli` feature stays as dependency-free as the library it wraps.

use std::fmt;

use charcode::{Bom, DecodeOptions, EncodeOptions, Encoding, Malformed, Unmappable};

pub const USAGE: &str = "\
Usage: charcode [OPTION]... [FILE]...
Convert text from one encoding to another.

Input and output encodings:
  -f, --from-code=NAME     encoding of the input (default UTF-8)
  -t, --to-code=NAME       encoding of the output (default UTF-8)

Error handling (the default is to stop at the first problem):
  -c                       omit malformed input and unmappable characters
      --substitute[=CHAR]  substitute instead: U+FFFD for malformed input, and
                           CHAR, by default '?', for a character the output
                           encoding cannot represent
      --translit           first try a close ASCII equivalent for such a
                           character: e for e-acute, oe for the ligature,
                           - for an em dash, EUR for the euro sign
      --html               write them as HTML numeric character references,
                           &#19968;, escaping & as &amp; so they read back
      --json               write them as JSON escapes, \\u4e00, escaping
                           backslash as well
      --bom=MODE           sniff (default), remove, or keep a byte order mark

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

A NAME may be any label of any charset this build carries, so \"latin1\",
\"ISO-8859-1\" and \"windows-1252\" all name the same encoding.  A Windows code
page number also works, written with its usual prefix: cp932, windows-1252,
ibm866, x-cp20936.  For compatibility with iconv,
a \"//IGNORE\" suffix on either NAME means the same as -c, and \"//TRANSLIT\"
the same as --translit.
";

/// What the program was asked to do.
#[derive(Debug)]
pub enum Command {
    Convert(Box<Options>),
    ListEncodings,
    ListLabels,
    Help,
    Version,
}

/// How to describe what happened to malformed input, for the stderr summary.
pub fn malformed_description(options: &DecodeOptions) -> &'static str {
    match options.malformed_policy() {
        Malformed::Fail => "rejected",
        Malformed::Omit => "omitted",
        Malformed::Replace(_) => "replaced",
    }
}

/// The same for characters the target encoding cannot represent.
pub fn unmappable_description(options: &EncodeOptions) -> &'static str {
    if options.transliterates() {
        return "transliterated where possible";
    }
    match options.unmappable_policy() {
        Unmappable::Fail => "rejected",
        Unmappable::Omit => "omitted",
        Unmappable::Replace(_) => "replaced",
        Unmappable::Html => "written as HTML numeric character references",
        Unmappable::JsonEscape => "written as JSON escapes",
    }
}

#[derive(Debug)]
pub struct Options {
    pub from: &'static Encoding,
    pub to: &'static Encoding,
    pub inputs: Vec<Input>,
    pub output: Option<String>,
    pub decode: DecodeOptions,
    pub encode: EncodeOptions,
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

/// What an iconv-style `//` suffix asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Suffix {
    None,
    Ignore,
    Translit,
}

/// Resolves a `-f`/`-t` value, honouring iconv's `//` suffixes.
fn resolve(kind: &str, value: &str) -> Result<(&'static Encoding, Suffix), ParseError> {
    let (name, suffix) = match value.split_once("//") {
        Some((name, suffix)) => (name, suffix),
        None => (value, ""),
    };
    let suffix = match suffix.to_ascii_uppercase().as_str() {
        "" => Suffix::None,
        "IGNORE" => Suffix::Ignore,
        "TRANSLIT" => Suffix::Translit,
        other => return error(format!("unknown suffix '//{other}' in {kind} '{value}'")),
    };
    Ok((resolve_name(kind, name)?, suffix))
}

/// Resolves a bare encoding name, by label first and then by code page.
fn resolve_name(kind: &str, name: &str) -> Result<&'static Encoding, ParseError> {
    if let Some(encoding) = Encoding::for_label_no_replacement(name.as_bytes()) {
        return Ok(encoding);
    }
    if let Some(encoding) = code_page(name)
        && Encoding::for_label_no_replacement(encoding.name().as_bytes()).is_some()
    {
        return Ok(encoding);
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
    let mut malformed = Malformed::Fail;
    let mut unmappable = Unmappable::Fail;
    let mut transliterate = false;
    let mut bom = Bom::Sniff;
    let mut suffix_ignore = false;
    let mut suffix_translit = false;
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
                    let (encoding, suffix) = resolve("input encoding", &value(&mut args)?)?;
                    from = encoding;
                    suffix_ignore |= suffix == Suffix::Ignore;
                    suffix_translit |= suffix == Suffix::Translit;
                }
                "to-code" => {
                    let (encoding, suffix) = resolve("output encoding", &value(&mut args)?)?;
                    to = encoding;
                    suffix_ignore |= suffix == Suffix::Ignore;
                    suffix_translit |= suffix == Suffix::Translit;
                }
                "output" => output = Some(value(&mut args)?),
                "substitute" => {
                    malformed = Malformed::default();
                    unmappable = Unmappable::Replace(match inline.as_deref() {
                        None => '?',
                        Some(text) => {
                            let mut chars = text.chars();
                            match (chars.next(), chars.next()) {
                                (Some(c), None) => c,
                                _ => {
                                    return error(
                                        "--substitute takes a single character, or nothing for '?'",
                                    );
                                }
                            }
                        }
                    });
                }
                "translit" => transliterate = true,
                "html" => {
                    malformed = Malformed::default();
                    unmappable = Unmappable::Html;
                }
                "json" => {
                    malformed = Malformed::default();
                    unmappable = Unmappable::JsonEscape;
                }
                "bom" => {
                    bom = match value(&mut args)?.as_str() {
                        "sniff" => Bom::Sniff,
                        "remove" => Bom::Remove,
                        "keep" | "ignore" => Bom::Ignore,
                        other => {
                            return error(format!(
                                "--bom takes sniff, remove or keep, not '{other}'"
                            ));
                        }
                    }
                }
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
                    let (encoding, suffix) = resolve("input encoding", &value(&mut args)?)?;
                    from = encoding;
                    suffix_ignore |= suffix == Suffix::Ignore;
                    suffix_translit |= suffix == Suffix::Translit;
                }
                't' => {
                    let (encoding, suffix) = resolve("output encoding", &value(&mut args)?)?;
                    to = encoding;
                    suffix_ignore |= suffix == Suffix::Ignore;
                    suffix_translit |= suffix == Suffix::Translit;
                }
                'o' => output = Some(value(&mut args)?),
                'c' => {
                    malformed = Malformed::Omit;
                    unmappable = Unmappable::Omit;
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

    if suffix_ignore && malformed == Malformed::Fail {
        malformed = Malformed::Omit;
        unmappable = Unmappable::Omit;
    }
    transliterate |= suffix_translit;
    if inputs.is_empty() {
        inputs.push(Input::Stdin);
    }
    Ok(Command::Convert(Box::new(Options {
        from,
        to,
        inputs,
        output,
        decode: DecodeOptions::new().bom(bom).malformed(malformed),
        encode: EncodeOptions::new()
            .unmappable(unmappable)
            .transliterate(transliterate),
        silent,
        verbose,
    })))
}

fn takes_value(long: &str) -> bool {
    matches!(
        long,
        "from-code" | "to-code" | "output" | "substitute" | "bom"
    )
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
        assert_eq!(options.decode.malformed_policy(), Malformed::Fail);
        assert_eq!(options.encode.unmappable_policy(), Unmappable::Fail);
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
        assert_eq!(options.encode.unmappable_policy(), Unmappable::Omit);
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
    fn error_policies() {
        assert_eq!(
            parse_options(&["--html"]).encode.unmappable_policy(),
            Unmappable::Html
        );
        assert_eq!(
            parse_options(&["--json"]).encode.unmappable_policy(),
            Unmappable::JsonEscape
        );
        assert_eq!(
            parse_options(&["--substitute=#"])
                .encode
                .unmappable_policy(),
            Unmappable::Replace('#')
        );
        assert!(parse_options(&["--translit"]).encode.transliterates());
        assert!(
            parse_options(&["-t", "ascii//TRANSLIT"])
                .encode
                .transliterates()
        );
        assert_eq!(
            parse_options(&["--bom=keep"]).decode.bom_handling(),
            Bom::Ignore
        );
        assert_eq!(
            parse_options(&["--bom", "remove"]).decode.bom_handling(),
            Bom::Remove
        );
        assert!(parse_args(&["--bom=nope"]).is_err());
        assert!(parse_args(&["--substitute=toolong"]).is_err());
    }

    #[test]
    fn iconv_suffixes() {
        let options = parse_options(&["-t", "ascii//IGNORE"]);
        assert_eq!(options.to, charcode::WINDOWS_1252);
        assert_eq!(options.encode.unmappable_policy(), Unmappable::Omit);
        // An explicit --substitute is not overridden by the suffix.
        let options = parse_options(&["--substitute", "-t", "ascii//IGNORE"]);
        assert_eq!(options.encode.unmappable_policy(), Unmappable::Replace('?'));
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
