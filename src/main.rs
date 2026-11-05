use clap::{Parser, ValueEnum};
use colored::{Color, Colorize};
use pretty_log::{format_line, is_valid_time_format, FormatOptions};
use std::io::{self, BufRead, Write};
use std::process::ExitCode;

fn parse_field_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

/// pretty-log parses JSON logs and shows them in a pretty format with colors easier to read for humans.
#[derive(Parser, Debug)]
#[command(version, about, author = "Blesswin Samuel")]
struct Opts {
    /// Field that represents time
    #[arg(short, long, default_value = "time,timestamp")]
    time_field: String,
    /// Field that represents level
    #[arg(short, long, default_value = "level,lvl")]
    level_field: String,
    /// Field that represents message
    #[arg(short, long, default_value = "message,msg")]
    message_field: String,
    /// Comma-separated non-core fields to include in the output; dotted paths select nested fields
    #[arg(long, default_value = "")]
    include_fields: String,
    /// Comma-separated non-core fields to hide from the output; dotted paths hide nested fields
    #[arg(long, default_value = "")]
    exclude_fields: String,
    /// Comma-separated preferred order for non-core fields; dotted paths order nested fields
    #[arg(long, default_value = "")]
    field_order: String,
    /// Display timestamps in UTC instead of the local timezone
    #[arg(long)]
    utc: bool,
    /// strftime-style format for timestamps, e.g. "%Y-%m-%d %H:%M:%S%.3f"
    #[arg(long)]
    time_format: Option<String>,
    /// When to use colorized output
    #[arg(long, value_enum, default_value_t = ColorMode::Auto)]
    color: ColorMode,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum ColorMode {
    Auto,
    Always,
    Never,
}

fn program_log(msg: &str) {
    eprintln!("{} {}", "<pretty-log>".color(Color::BrightBlack), msg)
}

fn main() -> ExitCode {
    let opts: Opts = Opts::parse();
    match opts.color {
        ColorMode::Auto => colored::control::unset_override(),
        ColorMode::Always => colored::control::set_override(true),
        ColorMode::Never => colored::control::set_override(false),
    }
    let include_fields = parse_field_list(&opts.include_fields);
    if let Some(format) = &opts.time_format {
        if !is_valid_time_format(format) {
            program_log(&format!("invalid --time-format: {format}"));
            return ExitCode::from(2);
        }
    }
    let format_opts = FormatOptions {
        time_field: opts.time_field.clone(),
        level_field: opts.level_field.clone(),
        message_field: opts.message_field.clone(),
        include_fields: (!include_fields.is_empty()).then(|| include_fields.iter().cloned().collect()),
        exclude_fields: parse_field_list(&opts.exclude_fields).into_iter().collect(),
        field_order: parse_field_list(&opts.field_order),
        utc: opts.utc,
        time_format: opts.time_format.clone(),
    };

    let input = io::stdin();
    let mut input = input.lock();
    let mut output = io::stdout().lock();
    let mut buf = Vec::new();
    let mut failed = false;
    loop {
        buf.clear();
        // Read raw bytes so lines that are not valid UTF-8 still reach the output.
        match input.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(_) => {}
            Err(err) => {
                program_log(&format!("Error: {err}"));
                failed = true;
                break;
            }
        }
        if buf.last() == Some(&b'\n') {
            buf.pop();
            if buf.last() == Some(&b'\r') {
                buf.pop();
            }
        }
        let line = String::from_utf8_lossy(&buf);
        match writeln!(output, "{}", format_line(&line, &format_opts)) {
            Ok(()) => {}
            // The consumer stopped reading, which is a normal way to end the stream.
            Err(err) if err.kind() == io::ErrorKind::BrokenPipe => break,
            Err(err) => {
                program_log(&format!("Error: {err}"));
                failed = true;
                break;
            }
        }
    }

    if let Err(err) = output.flush() {
        if !failed && err.kind() != io::ErrorKind::BrokenPipe {
            program_log(&format!("Error: {err}"));
            failed = true;
        }
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
