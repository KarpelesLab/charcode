//! `charcode` — convert text between the encodings of the WHATWG Encoding
//! Standard, with an interface modelled on `iconv`.

mod args;
mod convert;

use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::process::ExitCode;

use charcode::Encoding;

use crate::args::{Command, Input, Options, ParseError};
use crate::convert::{Converter, Error, Origin};

/// The size of one read from an input.  Conversion state carries across chunks,
/// so this only trades syscalls against memory.
const CHUNK: usize = 64 * 1024;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(message) => {
            eprintln!("charcode: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let command = args::parse(std::env::args().skip(1)).map_err(|ParseError(e)| e)?;
    match command {
        Command::Help => {
            print!("{}", args::USAGE);
            Ok(ExitCode::SUCCESS)
        }
        Command::Version => {
            println!("charcode {}", env!("CARGO_PKG_VERSION"));
            Ok(ExitCode::SUCCESS)
        }
        Command::ListEncodings => {
            let mut out = BufWriter::new(io::stdout().lock());
            for encoding in Encoding::all() {
                let _ = writeln!(out, "{}", encoding.name());
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::ListLabels => {
            let mut labels: Vec<(&str, &str)> = Encoding::all()
                .iter()
                .flat_map(|encoding| encoding.labels().map(move |l| (l, encoding.name())))
                .collect();
            labels.sort_unstable();
            let mut out = BufWriter::new(io::stdout().lock());
            for (label, name) in labels {
                let _ = writeln!(out, "{label}\t{name}");
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Convert(options) => convert(*options),
    }
}

fn convert(options: Options) -> Result<ExitCode, String> {
    let mut out: Box<dyn Write> = match &options.output {
        Some(path) => Box::new(BufWriter::new(
            File::create(path).map_err(|e| format!("{path}: {e}"))?,
        )),
        None => Box::new(BufWriter::new(io::stdout().lock())),
    };

    let mut converter = Converter::new(options.from, options.to, options.decode, options.encode);
    let mut buffer = vec![0u8; CHUNK];
    let last_input = options.inputs.len() - 1;

    for (index, input) in options.inputs.iter().enumerate() {
        let name = input.to_string();
        let mut reader: Box<dyn Read> = match input {
            Input::Stdin => Box::new(io::stdin().lock()),
            Input::File(path) => Box::new(File::open(path).map_err(|e| format!("{path}: {e}"))?),
        };
        let mut base = 0u64;
        loop {
            let read = match reader.read(&mut buffer) {
                Ok(read) => read,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(format!("{name}: {e}")),
            };
            let origin = Origin { name: &name, base };
            if read == 0 {
                // Flush the conversion state once, after the final input.
                if index == last_input {
                    report(converter.feed(&[], true, &mut out, &origin), &name)?;
                }
                break;
            }
            report(
                converter.feed(&buffer[..read], false, &mut out, &origin),
                &name,
            )?;
            base += read as u64;
        }
    }

    out.flush().map_err(|e| format!("write failed: {e}"))?;
    drop(out);

    summarize(&converter, &options);
    // Anything substituted or dropped means the output is not a faithful
    // conversion, which the exit status should say even though the run finished.
    let (decoded, encoded) = converter.tally();
    Ok(if decoded.is_lossless() && encoded.is_lossless() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn report(result: Result<(), Error>, name: &str) -> Result<(), String> {
    match result {
        Ok(()) => Ok(()),
        Err(Error::Io(e)) if e.kind() == io::ErrorKind::BrokenPipe => {
            // Downstream stopped reading; that is not a conversion failure.
            std::process::exit(0);
        }
        Err(Error::Io(e)) => Err(format!("{name}: {e}")),
        Err(Error::Convert(e)) => Err(e.to_string()),
    }
}

fn summarize(converter: &Converter, options: &Options) {
    let (decoded, encoded) = converter.tally();
    if options.verbose {
        eprintln!(
            "charcode: {} -> {}: {} byte(s) in, {} byte(s) out",
            options.from.name(),
            options.to.output_encoding().name(),
            converter.bytes_in,
            converter.bytes_out
        );
    }
    if options.silent {
        return;
    }
    if decoded.errors > 0 {
        eprintln!(
            "charcode: {} malformed byte sequence(s) were {}",
            decoded.errors,
            args::malformed_description(&options.decode)
        );
    }
    if encoded.errors > 0 {
        eprintln!(
            "charcode: {} character(s) that {} cannot represent were {}",
            encoded.errors,
            options.to.output_encoding().name(),
            args::unmappable_description(&options.encode)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_encoding_is_listed_under_a_usable_label() {
        // `--list` names must round-trip through `-f`/`-t`, except the three the
        // standard gives no encoder and `replacement`, which the parser rejects.
        for encoding in Encoding::all() {
            let found = Encoding::for_label(encoding.name().as_bytes());
            assert_eq!(found, Some(*encoding), "{}", encoding.name());
        }
    }
}
