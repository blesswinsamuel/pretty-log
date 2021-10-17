use chrono::prelude::*;
use clap::{AppSettings, Clap};
use colored::{Color, ColoredString, Colorize};
use regex::Regex;
use serde_json::Map;
use serde_json::{Result, Value};
use std::io::{self, BufRead};

/// This doc string acts as a help message when the user runs '--help'
/// as do all doc strings on fields
#[derive(Clap)]
#[clap(version = "1.0", author = "Blesswin Samuel")]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opts {
    /// Field that represents time
    #[clap(short, long, default_value = "time")]
    time_field: String,
    /// Field that represents level
    #[clap(short, long, default_value = "level")]
    level_field: String,
    /// Field that represents message
    #[clap(short, long, default_value = "message")]
    message_field: String,
}

fn main() {
    let opts: Opts = Opts::parse();

    let input = io::stdin();
    for line in input.lock().lines() {
        let l = match line {
            Ok(l) => l,
            Err(err) => {
                println!("{}", err);
                continue;
            }
        };
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
        let time_str = get_time(&obj, &opts.time_field);
        let level_str = get_level(&obj, &opts.level_field);
        let message_str = get_message(&obj, &opts.message_field);
        let fields_str = get_fields(&obj);
        println!("{} {} {} {}", time_str, level_str, message_str, fields_str);
    }
}

fn get_time(obj: &Map<String, Value>, key: &str) -> ColoredString {
    fn human_readable_date_from_string(s: &str) -> Option<DateTime<Local>> {
        let rfc3339 = DateTime::parse_from_rfc3339(s).ok();
        if rfc3339.is_some() {
            return rfc3339.map(|v| v.with_timezone(&Local));
        }
        None
    }
    fn human_readable_date_from_int(v: i64) -> Option<DateTime<Local>> {
        if v <= 1e11 as i64 {
            // 10 digits
            Some(Local.timestamp(v, 0))
        } else if v < 1e14 as i64 {
            // 13 digits
            Some(Local.timestamp_millis(v))
        } else {
            None
        }
    }
    let v = obj.get(key).unwrap_or(&Value::Null);
    let date = match v {
        Value::Number(n) => human_readable_date_from_int(n.as_i64().unwrap_or_default()),
        Value::String(s) => human_readable_date_from_string(s),
        _ => None,
    };
    match date {
        Some(d) => format!("{}", d.format("%H:%M:%S.%3f")),
        None => v.to_string(),
    }
    .color(Color::BrightBlack)
}

fn get_level(obj: &Map<String, Value>, key: &str) -> ColoredString {
    fn normalize_int_log_level(v: u64) -> String {
        match v {
            10 => "trace".to_string(),
            20 => "debug".to_string(),
            30 => "info".to_string(),
            40 => "warn".to_string(),
            50 => "error".to_string(),
            60 => "fatal".to_string(),
            _ => v.to_string(),
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
            _ => v.color(Color::BrightWhite).on_color(Color::Black).bold(),
        }
    }

    let v = obj.get(key).unwrap_or(&Value::Null);
    let normalized_level = match v {
        Value::Number(n) => normalize_int_log_level(n.as_u64().unwrap_or(0)),
        Value::String(s) => s.clone(),
        _ => "INVALID".to_string(),
    };
    return colorize_log_level(normalized_level.to_uppercase());
}

fn get_message(obj: &Map<String, Value>, key: &str) -> ColoredString {
    let v = obj.get(key).unwrap_or(&Value::Null);
    match v {
        Value::Number(n) => n.to_string().color(Color::White).bold(),
        Value::String(s) => s.clone().color(Color::White).bold(),
        _ => "".to_string().color(Color::White).bold(),
    }
}

fn get_fields(obj: &Map<String, Value>) -> String {
    fn get_field(k: &str, v: &Value) -> String {
        fn get_field_value(v: &Value) -> String {
            match v {
                Value::String(s) => format!(r#""{}""#, s).color(Color::BrightBlue).to_string(),
                Value::Number(n) => format!("{}", n).color(Color::BrightCyan).to_string(),
                Value::Bool(b) => format!("{}", b).color(Color::BrightGreen).to_string(),
                Value::Object(map) => {
                    let mut res: Vec<String> = vec![];
                    for (k, v) in map {
                        res.push(format!("{}={}", k.color(Color::BrightBlack), get_field_value(v)));
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
        res.push(get_field(k, f));
    }
    res.join(" ")
}
