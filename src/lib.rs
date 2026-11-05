use chrono::prelude::*;
use colored::{Color, ColoredString, Colorize};
use serde_json::Map;
use serde_json::{Result, Value};
use std::borrow::Cow;
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

    if obj.is_empty() {
        // An object with no fields renders nothing, so keep the original line instead.
        return line.to_string();
    }

    let (time_str, time_key) = get_time(obj, &opts.time_field);
    // Only pad the level when a time column precedes it, so a line without a time starts flush left.
    let (level_str, level_key) = get_level(obj, &opts.level_field, !time_str.is_empty());
    let (message_str, message_key) = get_message(obj, &opts.message_field);
    let mut excluded_fields: HashSet<String> = [time_key, level_key, message_key].iter().cloned().collect();
    excluded_fields.extend(opts.exclude_fields.iter().cloned());
    let fields_str = get_fields(obj, &excluded_fields, opts.include_fields.as_ref(), &opts.field_order);

    [time_str, level_str, message_str]
        .into_iter()
        .map(|part| part.to_string())
        .chain(std::iter::once(fields_str))
        .filter(|part| !part.is_empty())
        .collect::<Vec<String>>()
        .join(" ")
}

/// Renders control characters visibly so log content cannot drive the terminal.
fn escape_control_chars(value: &str) -> Cow<'_, str> {
    if !value.chars().any(char::is_control) {
        return Cow::Borrowed(value);
    }
    let mut escaped = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\t' => escaped.push(c),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            c if c.is_control() => escaped.push_str(&format!("\\u{{{:x}}}", u32::from(c))),
            c => escaped.push(c),
        }
    }
    Cow::Owned(escaped)
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

    fn human_readable_date_from_float(v: f64) -> Option<DateTime<Local>> {
        // Fractional timestamps are epoch seconds below 1e11 and epoch milliseconds above it.
        let seconds = if v.abs() <= 1e11 {
            v
        } else if v.abs() < 1e14 {
            v / 1000.0
        } else {
            return None;
        };
        let secs = seconds.floor() as i64;
        let nanos = ((seconds - seconds.floor()) * 1e9).round() as u32;
        // Rounding the fraction up to a full second must carry into the seconds.
        let (secs, nanos) = if nanos >= 1_000_000_000 { (secs + 1, 0) } else { (secs, nanos) };
        Local.timestamp_opt(secs, nanos).single()
    }

    for key in key.split(',') {
        let v = match obj.get(key) {
            Some(v) => v,
            None => continue,
        };
        let date = match v {
            Value::Number(n) => match n.as_i64() {
                Some(v) => human_readable_date_from_int(v),
                None => n.as_f64().and_then(human_readable_date_from_float),
            },
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
                Value::String(s) => escape_control_chars(s).into_owned(),
                _ => v.to_string(),
            },
        };
        return (v.color(Color::BrightBlack), key.to_string());
    }

    (ColoredString::default(), String::new())
}

fn get_level(obj: &Map<String, Value>, key: &str, pad: bool) -> (ColoredString, String) {
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

    fn normalize_string_log_level(v: &str) -> String {
        match v.to_lowercase().as_str() {
            "warning" => "warn".to_string(),
            "err" => "error".to_string(),
            "crit" | "critical" | "alert" | "emerg" | "emergency" => "fatal".to_string(),
            "notice" => "info".to_string(),
            other => other.to_string(),
        }
    }

    fn colorize_log_level(v: String, pad: bool) -> ColoredString {
        let pad_level = |s: String| if pad { format!("{:>5}", s) } else { s };
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
            Value::String(s) => normalize_string_log_level(&escape_control_chars(s)),
            _ => format!("invalid ({})", v),
        };
        return (colorize_log_level(normalized_level.to_uppercase(), pad), key.to_string());
    }

    (ColoredString::default(), String::new())
}

fn get_message(obj: &Map<String, Value>, key: &str) -> (ColoredString, String) {
    for key in key.split(',') {
        let v = match obj.get(key) {
            Some(v) => v,
            None => continue,
        };
        let v = match v {
            Value::Number(n) => n.to_string().color(Color::White).bold(),
            Value::String(s) => escape_control_chars(s).into_owned().color(Color::White).bold(),
            _ => v.to_string().color(Color::BrightRed).bold(),
        };
        return (v, key.to_string());
    }

    (ColoredString::default(), String::new())
}

struct FieldFilter<'a> {
    include: Option<&'a HashSet<String>>,
    exclude: &'a HashSet<String>,
    order: &'a [String],
}

impl FieldFilter<'_> {
    /// `path` is the dotted path of a field, such as `req.id` or `req`.
    fn is_visible(&self, path: &str) -> bool {
        if self.exclude.contains(path) {
            return false;
        }
        self.include.is_none_or(|include| {
            include
                .iter()
                .any(|selector| selector == path || is_ancestor(selector, path) || is_ancestor(path, selector))
        })
    }

    /// Position of `path` in `--field-order`; a selector for a descendant also positions its parents.
    fn order_index(&self, path: &str) -> Option<usize> {
        self.order.iter().position(|selector| selector == path || is_ancestor(path, selector))
    }
}

fn is_ancestor(ancestor: &str, path: &str) -> bool {
    path.len() > ancestor.len() && path.starts_with(ancestor) && path.as_bytes()[ancestor.len()] == b'.'
}

fn join_path(parent: &str, key: &str) -> String {
    if parent.is_empty() {
        key.to_string()
    } else {
        format!("{parent}.{key}")
    }
}

fn ordered_fields<'a>(parent: &str, obj: &'a Map<String, Value>, filter: &FieldFilter) -> Vec<(&'a String, &'a Value, String)> {
    let mut fields: Vec<(&String, &Value, String)> = obj
        .iter()
        .map(|(key, value)| (key, value, join_path(parent, key)))
        .filter(|(_, _, path)| filter.is_visible(path))
        .collect();
    // Stable: fields without a --field-order entry keep their input order.
    fields.sort_by_key(|(_, _, path)| filter.order_index(path).unwrap_or(usize::MAX));
    fields
}

fn get_fields(obj: &Map<String, Value>, exclude_fields: &HashSet<String>, include_fields: Option<&HashSet<String>>, field_order: &[String]) -> String {
    enum RenderedField {
        Inline(String),
        Block(String),
    }

    fn get_field_value(path: &str, v: &Value, filter: &FieldFilter) -> String {
        match v {
            Value::String(s) => format!(r#""{}""#, escape_control_chars(s)).color(Color::BrightBlue).to_string(),
            Value::Number(n) => format!("{}", n).color(Color::BrightCyan).to_string(),
            Value::Bool(b) => format!("{}", b).color(Color::BrightGreen).to_string(),
            Value::Object(map) => {
                let mut res: Vec<String> = vec![];
                for (k, v, child_path) in ordered_fields(path, map, filter) {
                    res.push(format!(
                        "{}{}{}",
                        escape_control_chars(k).color(Color::BrightBlack),
                        ":".color(Color::BrightYellow),
                        get_field_value(&child_path, v, filter)
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
                    res.push(get_field_value(path, v, filter));
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

    fn get_field(key: &str, v: &Value, path: &str, filter: &FieldFilter) -> RenderedField {
        if let Value::String(s) = v {
            if s.contains('\n') {
                let indented_lines = s
                    .lines()
                    .map(|line| format!("  {}", escape_control_chars(line)))
                    .collect::<Vec<String>>()
                    .join("\n");
                return RenderedField::Block(format!("{}:\n{}", escape_control_chars(key).color(Color::BrightBlack), indented_lines));
            }
        }

        RenderedField::Inline(format!(
            "{}={}",
            escape_control_chars(key).color(Color::BrightBlack),
            get_field_value(path, v, filter)
        ))
    }

    let filter = FieldFilter {
        include: include_fields,
        exclude: exclude_fields,
        order: field_order,
    };
    let mut inline_fields: Vec<String> = vec![];
    let mut block_fields: Vec<String> = vec![];

    for (key, value, path) in ordered_fields("", obj, &filter) {
        match get_field(key, value, &path, &filter) {
            RenderedField::Inline(field) => inline_fields.push(field),
            RenderedField::Block(field) => block_fields.push(field),
        }
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
    fn format_line_escapes_control_characters() {
        disable_colors();

        let line = r#"{"level":30,"message":"\u001b[31mred\u001b[0m","note":"tab\there","key\u001b":"v"}"#;
        let formatted = format_line(line, &test_options());

        assert!(!formatted.contains('\u{1b}'), "escape sequence reached the output: {formatted:?}");
        assert!(formatted.contains("\\u{1b}[31mred"), "{formatted:?}");
        assert!(formatted.contains("tab\there"), "{formatted:?}");
        assert!(formatted.contains("key\\u{1b}=\"v\""), "{formatted:?}");
    }

    #[test]
    fn format_line_maps_level_synonyms() {
        disable_colors();

        for (line, expected) in [
            (r#"{"level":"warning","message":"x"}"#, "WARN x"),
            (r#"{"level":"WARNING","message":"x"}"#, "WARN x"),
            (r#"{"level":"err","message":"x"}"#, "ERROR x"),
            (r#"{"level":"critical","message":"x"}"#, "FATAL x"),
            (r#"{"level":"notice","message":"x"}"#, "INFO x"),
        ] {
            assert_eq!(format_line(line, &test_options()), expected);
        }
    }

    #[test]
    fn get_time_parses_fractional_timestamps() {
        disable_colors();

        for (value, equivalent_millis) in [(json!(1624829360.868), 1624829360868_i64), (json!(1624829360868.5), 1624829360868_i64)] {
            let fractional = json!({ "time": value });
            let whole = json!({ "time": equivalent_millis });

            let (fractional, _) = get_time(fractional.as_object().unwrap(), "time");
            let (whole, _) = get_time(whole.as_object().unwrap(), "time");

            assert_eq!(fractional.to_string(), whole.to_string());
        }
    }

    #[test]
    fn get_time_keeps_timestamps_it_cannot_parse() {
        disable_colors();

        let line = r#"{"time":18446744073709551615,"level":30,"message":"hello"}"#;
        let formatted = format_line(line, &test_options());

        assert!(formatted.contains("18446744073709551615"), "{formatted:?}");
        assert!(!formatted.contains("1970"), "{formatted:?}");
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
            &["time".to_string(), "level".to_string(), "message".to_string()].iter().cloned().collect(),
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
    fn format_line_keeps_input_field_order() {
        disable_colors();

        let line = r#"{"z_field":1,"a_field":2,"payload":{"z":1,"a":2}}"#;
        let formatted = format_line(line, &test_options());

        assert_eq!(formatted, "z_field=1 a_field=2 payload={z:1, a:2}");
    }

    #[test]
    fn format_line_omits_absent_core_fields() {
        disable_colors();

        let line = r#"{"message":"hello","request_id":42}"#;
        let formatted = format_line(line, &test_options());

        assert_eq!(formatted, "hello request_id=42");
    }

    #[test]
    fn format_line_renders_missing_message_as_blank() {
        disable_colors();

        let line = r#"{"time":1624829360868,"level":30,"host":"box"}"#;
        let formatted = format_line(line, &test_options());

        assert!(formatted.ends_with("INFO host=\"box\""), "{formatted:?}");
        assert!(!formatted.contains("null"), "{formatted:?}");
    }

    #[test]
    fn format_line_does_not_pad_level_without_time() {
        disable_colors();

        let line = r#"{"level":30,"message":"hello"}"#;
        assert_eq!(format_line(line, &test_options()), "INFO hello");
    }

    #[test]
    fn format_line_passes_through_empty_object() {
        disable_colors();

        assert_eq!(format_line("{}", &test_options()), "{}");
    }

    #[test]
    fn format_line_does_not_emit_trailing_whitespace() {
        disable_colors();

        for line in [
            r#"{"time":1624829360868,"level":30}"#,
            r#"{"time":1624829360868,"level":30,"message":""}"#,
            r#"{"time":1624829360868,"level":30,"message":"hi"}"#,
            r#"{"message":"hi","type":"Error","stack":"a\nb"}"#,
            "{}",
        ] {
            let formatted = format_line(line, &test_options());
            assert_eq!(formatted.trim_end(), formatted, "trailing whitespace in {formatted:?}");
        }
    }

    #[test]
    fn include_fields_selects_nested_keys() {
        disable_colors();

        let mut opts = test_options();
        opts.include_fields = Some(["req.id".to_string(), "service".to_string()].iter().cloned().collect());

        let line = r#"{"level":30,"message":"hello","service":"api","req":{"id":"r1","method":"POST"}}"#;
        let formatted = format_line(line, &opts);

        assert_eq!(formatted, "INFO hello service=\"api\" req={id:\"r1\"}");
    }

    #[test]
    fn include_fields_accepts_subtree_selectors() {
        disable_colors();

        let mut opts = test_options();
        opts.include_fields = Some(["req".to_string()].iter().cloned().collect());

        let line = r#"{"level":30,"message":"hello","service":"api","req":{"id":"r1","method":"POST"}}"#;
        let formatted = format_line(line, &opts);

        assert_eq!(formatted, "INFO hello req={id:\"r1\", method:\"POST\"}");
    }

    #[test]
    fn exclude_fields_hides_nested_keys() {
        disable_colors();

        let mut opts = test_options();
        opts.exclude_fields = ["req.method".to_string()].iter().cloned().collect();

        let line = r#"{"level":30,"message":"hello","req":{"id":"r1","method":"POST"}}"#;
        let formatted = format_line(line, &opts);

        assert_eq!(formatted, "INFO hello req={id:\"r1\"}");
    }

    #[test]
    fn field_order_accepts_nested_paths() {
        disable_colors();

        let mut opts = test_options();
        opts.field_order = vec!["req.method".to_string()];

        let line = r#"{"level":30,"message":"hello","req":{"id":"r1","method":"POST"}}"#;
        let formatted = format_line(line, &opts);

        assert_eq!(formatted, "INFO hello req={method:\"POST\", id:\"r1\"}");
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
