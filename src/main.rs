use clap::Parser;
use colored::{Color, Colorize};
use pretty_log::{format_line, FormatOptions};
use signal_hook::iterator::Signals;
use std::io::{self, BufRead};
use std::os::raw::c_int;
use std::thread;

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
}

fn program_log(msg: &str) {
    eprintln!("{} {}", "<pretty-log>".color(Color::BrightBlack), msg)
}

fn main() {
    std::env::set_var("CLICOLOR_FORCE", "1");

    let opts: Opts = Opts::parse();
    let format_opts = FormatOptions {
        time_field: opts.time_field.clone(),
        level_field: opts.level_field.clone(),
        message_field: opts.message_field.clone(),
    };
    thread::scope(|s| {
        const SIGNALS: &[c_int] = &[
            signal_hook::consts::SIGHUP,
            signal_hook::consts::SIGINT,
            signal_hook::consts::SIGTERM,
            signal_hook::consts::SIGQUIT,
        ];
        let mut sigs = Signals::new(SIGNALS).unwrap();
        let sigs_handle = sigs.handle();
        s.spawn(move || {
            for signal in &mut sigs {
                program_log(&format!("Received signal {:?}", signal));
                // After printing it, do whatever the signal was supposed to do in the first place
                // low_level::emulate_default_handler(signal).unwrap();
                break;
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
