use chrono::prelude::*;
use colored::{Color, ColoredString, Colorize};
use serde_json::Map;
use serde_json::{Result, Value};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct FormatOptions {
    pub time_field: String,
    pub level_field: String,
    pub message_field: String,
    pub include_fields: Option<HashSet<String>>,
    pub exclude_fields: HashSet<String>,
    pub field_order: Vec<String>,
}

pub fn format_line(line: &str, opts: &FormatOptions) -> String {
    let parsed = match serde_json::from_str(line) as Result<Value> {
        Ok(parsed) => parsed,
        Err(_) => return line.to_string(),
    };

    let obj = match parsed.as_object() {
        Some(obj) => obj,
        None => return line.to_string(),
    };

    let (time_str, time_key) = get_time(obj, &opts.time_field);
    let (level_str, level_key) = get_level(obj, &opts.level_field);
    let (message_str, message_key) = get_message(obj, &opts.message_field);
    let mut excluded_fields: HashSet<String> = [time_key, level_key, message_key].iter().cloned().collect();
    excluded_fields.extend(opts.exclude_fields.iter().cloned());
    let fields_str = get_fields(obj, &excluded_fields, opts.include_fields.as_ref(), &opts.field_order);

    format!("{} {} {} {}", time_str, level_str, message_str, fields_str)
}

fn get_time(obj: &Map<String, Value>, key: &str) -> (ColoredString, String) {
    fn human_readable_date_from_string(s: &str) -> Option<DateTime<Local>> {
        dateparser::parse_with_timezone(s, &Local).map(|v| v.with_timezone(&Local)).ok()
    }

    fn human_readable_date_from_int(v: i64) -> Option<DateTime<Local>> {
        if v <= 1e11 as i64 {
            Local.timestamp_opt(v, 0).single()
        } else if v < 1e14 as i64 {
            Local.timestamp_millis_opt(v).single()
        } else {
            None
        }
    }

    for key in key.split(',') {
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
            Some(d) if d.day() != now.day() || d.month() != now.month() || d.year() != now.year() => {
                format!("{}", d.format("%Y-%m-%d %H:%M:%S.%3f"))
            }
            Some(d) => format!("{}", d.format("%H:%M:%S.%3f")),
            None => match v {
                Value::Number(n) => n.to_string(),
                Value::String(s) => s.to_string(),
                _ => v.to_string(),
            },
        };
        return (v.color(Color::BrightBlack), key.to_string());
    }

    ("EMPTY TIME".color(Color::BrightBlack), String::new())
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

    for key in key.split(',') {
        let v = match obj.get(key) {
            Some(v) => v,
            None => continue,
        };
        let normalized_level = match v {
            Value::Number(n) => normalize_int_log_level(n.as_u64().unwrap_or(0)),
            Value::String(s) => s.clone(),
            _ => format!("invalid ({})", v),
        };
        return (colorize_log_level(normalized_level.to_uppercase()), key.to_string());
    }

    ("EMPTY".color(Color::BrightBlack).on_color(Color::BrightYellow).bold(), String::new())
}

fn get_message(obj: &Map<String, Value>, key: &str) -> (ColoredString, String) {
    for key in key.split(',') {
        let v = match obj.get(key) {
            Some(v) => v,
            None => continue,
        };
        let v = match v {
            Value::Number(n) => n.to_string().color(Color::White).bold(),
            Value::String(s) => s.clone().color(Color::White).bold(),
            _ => v.to_string().color(Color::BrightRed).bold(),
        };
        return (v, key.to_string());
    }

    ("null".color(Color::BrightRed).bold(), String::new())
}

fn get_fields(
    obj: &Map<String, Value>,
    exclude_fields: &HashSet<String>,
    include_fields: Option<&HashSet<String>>,
    field_order: &[String],
) -> String {
    enum RenderedField {
        Inline(String),
        Block(String),
    }

    fn get_field(k: &str, v: &Value) -> RenderedField {
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
                    for v in array {
                        res.push(get_field_value(v));
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

        if let Value::String(s) = v {
            if s.contains('\n') {
                let indented_lines = s.lines().map(|line| format!("  {}", line)).collect::<Vec<String>>().join("\n");
                return RenderedField::Block(format!("{}:\n{}", k.color(Color::BrightBlack), indented_lines));
            }
        }

        RenderedField::Inline(format!("{}={}", k.color(Color::BrightBlack), get_field_value(v)))
    }

    fn is_visible(k: &str, include_fields: Option<&HashSet<String>>, exclude_fields: &HashSet<String>) -> bool {
        if exclude_fields.contains(k) {
            return false;
        }
        include_fields.map(|fields| fields.contains(k)).unwrap_or(true)
    }

    let mut inline_fields: Vec<String> = vec![];
    let mut block_fields: Vec<String> = vec![];
    let mut rendered_keys: HashSet<String> = HashSet::new();

    let mut push_field = |k: &str, f: &Value| match get_field(k, f) {
            RenderedField::Inline(field) => inline_fields.push(field),
            RenderedField::Block(field) => block_fields.push(field),
        };

    for key in field_order {
        if rendered_keys.contains(key) || !is_visible(key, include_fields, exclude_fields) {
            continue;
        }
        if let Some(value) = obj.get(key) {
            push_field(key, value);
            rendered_keys.insert(key.clone());
        }
    }

    for (k, f) in obj {
        if rendered_keys.contains(k) || !is_visible(k, include_fields, exclude_fields) {
            continue;
        }
        push_field(k, f);
        rendered_keys.insert(k.clone());
    }

    match (inline_fields.is_empty(), block_fields.is_empty()) {
        (true, true) => String::new(),
        (false, true) => inline_fields.join(" "),
        (true, false) => block_fields.join("\n"),
        (false, false) => format!("{}\n{}", inline_fields.join(" "), block_fields.join("\n")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn disable_colors() {
        colored::control::set_override(false);
    }

    fn test_options() -> FormatOptions {
        FormatOptions {
            time_field: "time,timestamp".to_string(),
            level_field: "level,lvl".to_string(),
            message_field: "message,msg".to_string(),
            include_fields: None,
            exclude_fields: HashSet::new(),
            field_order: Vec::new(),
        }
    }

    #[test]
    fn format_line_passes_through_plain_text() {
        disable_colors();

        assert_eq!(format_line("plain text log", &test_options()), "plain text log");
    }

    #[test]
    fn format_line_passes_through_non_object_json() {
        disable_colors();

        assert_eq!(format_line("[1,true,\"hello\"]", &test_options()), "[1,true,\"hello\"]");
    }

    #[test]
    fn format_line_uses_alias_fields() {
        disable_colors();

        let line = r#"{"timestamp":"2021-04-17T09:45:32.137Z","lvl":"info","msg":"hello","user":"sam"}"#;
        let formatted = format_line(line, &test_options());

        assert!(formatted.contains("INFO"));
        assert!(formatted.contains("hello"));
        assert!(formatted.contains("user=\"sam\""));
        assert!(!formatted.contains("lvl="));
        assert!(!formatted.contains("msg="));
        assert!(!formatted.contains("timestamp="));
    }

    #[test]
    fn format_line_normalizes_numeric_levels() {
        disable_colors();

        let line = r#"{"time":1624829360868,"level":50,"message":"boom"}"#;
        let formatted = format_line(line, &test_options());

        assert!(formatted.contains("ERROR"));
        assert!(formatted.contains("boom"));
    }

    #[test]
    fn get_fields_skips_excluded_keys() {
        disable_colors();

        let obj = json!({
            "time": "2021-04-17T09:45:32.137Z",
            "level": "info",
            "message": "hello",
            "request_id": 42,
            "ok": true
        });
        let fields = get_fields(
            obj.as_object().unwrap(),
            &["time".to_string(), "level".to_string(), "message".to_string()]
                .iter()
                .cloned()
                .collect(),
            None,
            &[],
        );

        assert!(fields.contains("request_id=42"));
        assert!(fields.contains("ok=true"));
    }

    #[test]
    fn format_line_renders_multiline_fields_as_blocks() {
        disable_colors();

        let line = r#"{"time":1624829360868,"level":50,"message":"boom","type":"Error","stack":"Error: boom\n    at main"}"#;
        let formatted = format_line(line, &test_options());

        assert!(formatted.contains("ERROR boom type=\"Error\""));
        assert!(formatted.contains("\nstack:\n  Error: boom\n      at main"));
    }

    #[test]
    fn format_line_applies_field_filters_and_ordering() {
        disable_colors();

        let mut opts = test_options();
        opts.include_fields = Some(["request_id".to_string(), "service".to_string()].iter().cloned().collect());
        opts.exclude_fields = ["service".to_string()].iter().cloned().collect();
        opts.field_order = vec!["service".to_string(), "request_id".to_string()];

        let line = r#"{"time":1624829360868,"level":30,"message":"hello","hostname":"box","request_id":42,"service":"api"}"#;
        let formatted = format_line(line, &opts);

        assert!(formatted.contains("INFO hello request_id=42"));
        assert!(!formatted.contains("hostname="));
        assert!(!formatted.contains("service="));
    }
}