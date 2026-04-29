use clap::{Parser, ValueEnum};
use colored::{Color, Colorize};
use pretty_log::{format_line, FormatOptions};
use signal_hook::iterator::Signals;
use std::io::{self, BufRead};
use std::os::raw::c_int;
use std::thread;

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
    /// Comma-separated non-core fields to include in the output
    #[arg(long, default_value = "")]
    include_fields: String,
    /// Comma-separated non-core fields to hide from the output
    #[arg(long, default_value = "")]
    exclude_fields: String,
    /// Comma-separated preferred order for non-core fields
    #[arg(long, default_value = "")]
    field_order: String,
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

fn main() {
    let opts: Opts = Opts::parse();
    match opts.color {
        ColorMode::Auto => colored::control::unset_override(),
        ColorMode::Always => colored::control::set_override(true),
        ColorMode::Never => colored::control::set_override(false),
    }
    let include_fields = parse_field_list(&opts.include_fields);
    let format_opts = FormatOptions {
        time_field: opts.time_field.clone(),
        level_field: opts.level_field.clone(),
        message_field: opts.message_field.clone(),
        include_fields: (!include_fields.is_empty()).then(|| include_fields.iter().cloned().collect()),
        exclude_fields: parse_field_list(&opts.exclude_fields).into_iter().collect(),
        field_order: parse_field_list(&opts.field_order),
    };
    const SIGNALS: &[c_int] = &[
        signal_hook::consts::SIGHUP,
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGQUIT,
    ];
    let mut sigs = match Signals::new(SIGNALS) {
        Ok(sigs) => sigs,
        Err(err) => {
            program_log(&format!("Failed to install signal handlers: {:?}", err));
            std::process::exit(1);
        }
    };

    thread::scope(|s| {
        let sigs_handle = sigs.handle();
        s.spawn(move || {
            if let Some(signal) = (&mut sigs).into_iter().next() {
                program_log(&format!("Received signal {:?}", signal));
                // After printing it, do whatever the signal was supposed to do in the first place
                // low_level::emulate_default_handler(signal).unwrap();
            }
        });
        s.spawn(move || {
            let input = io::stdin();
            for line in input.lock().lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(err) => {
                        program_log(&format!("Error: {:?}", err));
                        continue;
                    }
                };
                println!("{}", format_line(&line, &format_opts));
            }
            program_log("Stopping");
            sigs_handle.close();
        });
    });
}
