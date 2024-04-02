use chrono::prelude::*;
use clap::Parser;
use colored::{Color, ColoredString, Colorize};
use serde_json::Map;
use serde_json::{Result, Value};
use signal_hook::consts::TERM_SIGNALS;
use signal_hook::flag;
use signal_hook::iterator::exfiltrator::WithOrigin;
use signal_hook::iterator::{Signals, SignalsInfo};
use std::io::{self, BufRead};
use std::os::raw::c_int;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;

/// This doc string acts as a help message when the user runs '--help'
/// as do all doc strings on fields
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
    eprintln!("{} {}", "<pretty-json-log>".color(Color::BrightBlack), msg)
}

enum Message {
    Log(String),
    Signal(c_int),
}

fn main() {
    let opts: Opts = Opts::parse();
    thread::scope(|s| {
        let (tx, rx): (Sender<Message>, Receiver<Message>) = mpsc::channel();

        let tx1 = tx.clone();
        let term_now = Arc::new(AtomicBool::new(false));
        for sig in TERM_SIGNALS {
            // When terminated by a second term signal, exit with exit code 1.
            // This will do nothing the first time (because term_now is false).
            flag::register_conditional_shutdown(*sig, 1, Arc::clone(&term_now)).unwrap();
            // But this will "arm" the above for the second time, by setting it to true.
            // The order of registering these is important, if you put this one first, it will
            // first arm and then terminate ‒ all in the first round.
            flag::register(*sig, Arc::clone(&term_now)).unwrap();
        }

        // const SIGNALS: &[c_int] = &[
        //     signal_hook::consts::SIGHUP,
        //     signal_hook::consts::SIGINT,
        //     signal_hook::consts::SIGTERM,
        //     signal_hook::consts::SIGQUIT,
        // ];
        // Subscribe to all these signals with information about where they come from. We use the
        // extra info only for logging in this example (it is not available on all the OSes or at
        // all the occasions anyway, it may return `Unknown`).
        let mut sigs: Vec<i32> = vec![
            signal_hook::consts::SIGUSR1,
            // // Some terminal handling
            // SIGTSTP, SIGCONT, SIGWINCH,
            // // Reload of configuration for daemons ‒ um, is this example for a TUI app or a daemon
            // // O:-)? You choose...
            // SIGHUP, // Application-specific action, to print some statistics.
            // SIGUSR1,
        ];
        sigs.extend(TERM_SIGNALS);
        let mut signals = SignalsInfo::<WithOrigin>::new(&sigs).unwrap();
        let signals_handle = signals.handle();
        s.spawn(move || {
            // let mut has_terminal = true;
            for info in &mut signals {
                // Will print info about signal + where it comes from.
                eprintln!("Received a signal {:?}", info);
                match info.signal {
                    signal_hook::consts::SIGUSR1 => break,
                    // SIGTSTP => {
                    //     // Restore the terminal to non-TUI mode
                    //     if has_terminal {
                    //         app.restore_term();
                    //         has_terminal = false;
                    //         // And actually stop ourselves.
                    //         low_level::emulate_default_handler(SIGTSTP)?;
                    //     }
                    // }
                    // SIGCONT => {
                    //     if !has_terminal {
                    //         app.claim_term();
                    //         has_terminal = true;
                    //     }
                    // }
                    // SIGWINCH => app.resize_term(),
                    // SIGHUP => app.reload_config(),
                    // SIGUSR1 => app.print_stats(),
                    term_sig => {
                        // These are all the ones left
                        // program_log(&format!("Received signal {:?}", signal));
                        tx1.send(Message::Signal(term_sig)).unwrap();
                        // After printing it, do whatever the signal was supposed to do in the first place
                        // low_level::emulate_default_handler(signal).unwrap();
                        break;
                    }
                }
            }
            program_log("Stopping thread 1")
        });
        let tx2 = tx.clone();
        s.spawn(move || {
            let input = io::stdin();
            for line in input.lock().lines() {
                let l = match line {
                    Ok(l) => l,
                    Err(err) => {
                        program_log(&format!("Error: {:?}", err));
                        continue;
                    }
                };
                tx2.send(Message::Log(l)).unwrap();
            }
            tx2.send(Message::Signal(-1)).unwrap();
            program_log("Stopping thread 2")
        });
        s.spawn(move || {
            for msg in rx {
                match msg {
                    Message::Log(l) => {
                        let s = match serde_json::from_str(&l) as Result<Value> {
                            Ok(s) => s,
                            Err(_) => {
                                println!("{}", l);
                                continue;
                            }
                        };
                        let obj = match s.as_object() {
                            Some(v) => v,
                            None => {
                                println!("None");
                                continue;
                            }
                        };
                        let (time_str, time_key) = get_time(&obj, &opts.time_field);
                        let (level_str, level_key) = get_level(&obj, &opts.level_field);
                        let (message_str, message_key) = get_message(&obj, &opts.message_field);
                        let fields_str = get_fields(&obj, [time_key, level_key, message_key].iter().cloned().collect());
                        println!("{} {} {} {}", time_str, level_str, message_str, fields_str);
                    }
                    Message::Signal(signal) => {
                        program_log(&format!("Received signal {:?}", signal));
                        break;
                    }
                }
            }
            program_log("Stopping");
            signals_handle.close();
        });
    });
}

fn get_time(obj: &Map<String, Value>, key: &str) -> (ColoredString, String) {
    fn human_readable_date_from_string(s: &str) -> Option<DateTime<Local>> {
        dateparser::parse_with_timezone(s, &Local).map(|v| v.with_timezone(&Local)).ok()
    }
    fn human_readable_date_from_int(v: i64) -> Option<DateTime<Local>> {
        if v <= 1e11 as i64 {
            // 10 digits
            Local.timestamp_opt(v, 0).single()
        } else if v < 1e14 as i64 {
            // 13 digits
            Local.timestamp_millis_opt(v).single()
        } else {
            None
        }
    }
    for key in key.split(",") {
        let v = match obj.get(key) {
            Some(v) => v,
            None => continue,
        };
        let date = match v {
            Value::Number(n) => human_readable_date_from_int(n.as_i64().unwrap_or_default()),
            Value::String(s) => human_readable_date_from_string(s),
            _ => None,
        };
        let now = Local::now();
        let v = match date {
            Some(d) if d.day() != now.day() || d.month() != now.month() || d.year() != now.year() => format!("{}", d.format("%Y-%m-%d %H:%M:%S.%3f")),
            Some(d) => format!("{}", d.format("%H:%M:%S.%3f")),
            None => match v {
                Value::Number(n) => n.to_string(),
                Value::String(s) => s.to_string(),
                _ => v.to_string(),
            },
        };
        return (v.color(Color::BrightBlack), key.to_string());
    }
    return ("EMPTY TIME".color(Color::BrightBlack), "".to_string());
}

fn get_level(obj: &Map<String, Value>, key: &str) -> (ColoredString, String) {
    fn normalize_int_log_level(v: u64) -> String {
        match v {
            10 => "trace".to_string(),
            20 => "debug".to_string(),
            30 => "info".to_string(),
            40 => "warn".to_string(),
            50 => "error".to_string(),
            60 => "fatal".to_string(),
            _ => format!("UNKNOWN ({})", v),
        }
    }
    fn colorize_log_level(v: String) -> ColoredString {
        let pad_level = |s: String| format!("{:>5}", s);
        match v.as_str() {
            "PANIC" => pad_level(v).color(Color::Red).on_color(Color::BrightWhite).bold(),
            "FATAL" => pad_level(v).color(Color::BrightWhite).on_color(Color::Red).bold(),
            "ERROR" => pad_level(v).color(Color::BrightWhite).on_color(Color::BrightRed).bold(),
            "WARN" => pad_level(v).color(Color::BrightBlack).on_color(Color::BrightYellow).bold(),
            "INFO" => pad_level(v).color(Color::BrightWhite).on_color(Color::BrightBlue).bold(),
            "DEBUG" => pad_level(v).color(Color::BrightWhite).on_color(Color::BrightBlack).bold(),
            "TRACE" => pad_level(v).color(Color::BrightWhite).on_color(Color::Black).bold(),
            _ => v.color(Color::BrightWhite).on_color(Color::BrightBlack).bold(),
        }
    }

    for key in key.split(",") {
        let v = obj.get(key);
        if v.is_none() {
            continue;
        }
        let v = v.unwrap_or(&Value::Null);
        let normalized_level = match v {
            Value::Number(n) => normalize_int_log_level(n.as_u64().unwrap_or(0)),
            Value::String(s) => s.clone(),
            _ => format!("invalid ({})", v),
        };
        return (colorize_log_level(normalized_level.to_uppercase()), key.to_string());
    }
    return ("EMPTY".color(Color::BrightBlack).on_color(Color::BrightYellow).bold(), "".to_string());
}

fn get_message(obj: &Map<String, Value>, key: &str) -> (ColoredString, String) {
    for key in key.split(",") {
        let v = obj.get(key);
        if v.is_none() {
            continue;
        }
        let v = v.unwrap_or(&Value::Null);
        let v = match v {
            Value::Number(n) => n.to_string().color(Color::White).bold(),
            Value::String(s) => s.clone().color(Color::White).bold(),
            _ => v.to_string().color(Color::BrightRed).bold(),
        };
        return (v, key.to_string());
    }
    return ("null".color(Color::BrightRed).bold(), "".to_string());
}

fn get_fields(obj: &Map<String, Value>, exclude_fields: std::collections::HashSet<String>) -> String {
    fn get_field(k: &str, v: &Value) -> String {
        fn get_field_value(v: &Value) -> String {
            match v {
                Value::String(s) => format!(r#""{}""#, s).color(Color::BrightBlue).to_string(),
                Value::Number(n) => format!("{}", n).color(Color::BrightCyan).to_string(),
                Value::Bool(b) => format!("{}", b).color(Color::BrightGreen).to_string(),
                Value::Object(map) => {
                    let mut res: Vec<String> = vec![];
                    for (k, v) in map {
                        res.push(format!(
                            "{}{}{}",
                            k.color(Color::BrightBlack),
                            ":".color(Color::BrightYellow),
                            get_field_value(v)
                        ));
                    }
                    format!(
                        "{}{}{}",
                        "{".color(Color::BrightYellow),
                        res.join(", ".color(Color::BrightYellow).to_string().as_str()),
                        "}".color(Color::BrightYellow)
                    )
                }
                Value::Array(array) => {
                    let mut res: Vec<String> = vec![];
                    for v in array.iter() {
                        res.push(format!("{}", get_field_value(v)));
                    }
                    format!(
                        "{}{}{}",
                        "[".color(Color::BrightMagenta),
                        res.join(", ".color(Color::BrightMagenta).to_string().as_str()),
                        "]".color(Color::BrightMagenta)
                    )
                }
                Value::Null => "null".color(Color::BrightRed).to_string(),
            }
        }
        format!("{}={}", k.color(Color::BrightBlack), get_field_value(v))
    }
    let mut res: Vec<String> = vec![];
    for (k, f) in obj {
        if exclude_fields.contains(k) {
            continue;
        }
        res.push(get_field(k, f));
    }
    res.join(" ")
}
