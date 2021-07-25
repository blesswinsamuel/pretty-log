use clap::{AppSettings, Clap};
use serde_json::json;
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
}

fn main() {
    let opts: Opts = Opts::parse();

    let input = io::stdin();
    for line in input.lock().lines() {
        match line {
            Ok(l) => {
                let v: Result<Value> = serde_json::from_str(&l);
                match v {
                    Ok(s) => match s.as_object() {
                        Some(obj) => {
                            println!("{}", obj[&opts.level_field]);
                        }
                        None => {
                            println!("None")
                        }
                    },
                    Err(_) => {
                        println!("{}", l)
                    }
                }
                // println!("{}", v);

                // let obj: Map<String, Value> = v.as_object().unwrap().clone();
            }
            Err(err) => {
                println!("{}", err);
            }
        }
    }
}
