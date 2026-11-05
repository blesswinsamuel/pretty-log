use chrono::prelude::*;
use colored::{Color, ColoredString, Colorize};
use serde_json::Map;
use serde_json::{Result, Value};
use std::borrow::Cow;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct FormatOptions {
    pub time_field: Vec<String>,
    pub level_field: Vec<String>,
    pub message_field: Vec<String>,
    pub include_fields: Option<HashSet<String>>,
    pub exclude_fields: HashSet<String>,
    pub field_order: Vec<String>,
    pub utc: bool,
    pub time_format: Option<String>,
}

/// Whether `format` is a usable chrono strftime format string.
pub fn is_valid_time_format(format: &str) -> bool {
    !chrono::format::StrftimeItems::new(format).any(|item| matches!(item, chrono::format::Item::Error))
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

    let (time_str, time_key) = get_time(obj, opts);
    // Only pad the level when a time column precedes it, so a line without a time starts flush left.
    let (level_str, level_key) = get_level(obj, &opts.level_field, !time_str.is_empty());
    let (message_lines, message_key) = get_message(obj, &opts.message_field);
    let mut excluded_fields: HashSet<String> = [time_key, level_key, message_key].iter().cloned().collect();
    excluded_fields.extend(opts.exclude_fields.iter().cloned());
    let (inline_fields, block_fields) = get_fields(obj, &excluded_fields, opts.include_fields.as_ref(), &opts.field_order);

    // A multiline message keeps its first line in the header and continues as an indented block.
    let message_first = message_lines.first().map(|line| line.to_string()).unwrap_or_default();
    let header = [time_str.to_string(), level_str.to_string(), message_first, inline_fields]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<String>>()
        .join(" ");

    let mut lines: Vec<String> = Vec::new();
    if !header.is_empty() {
        lines.push(header);
    }
    lines.extend(message_lines.iter().skip(1).map(|line| format!("  {line}")));
    if !block_fields.is_empty() {
        lines.push(block_fields);
    }

    lines.join("\n")
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

fn get_time(obj: &Map<String, Value>, opts: &FormatOptions) -> (ColoredString, String) {
    fn date_from_seconds(seconds: i64, nanos: u32, utc: bool) -> Option<DateTime<FixedOffset>> {
        if utc {
            Utc.timestamp_opt(seconds, nanos).single().map(|d| d.fixed_offset())
        } else {
            Local.timestamp_opt(seconds, nanos).single().map(|d| d.fixed_offset())
        }
    }

    fn date_from_string(s: &str, utc: bool) -> Option<DateTime<FixedOffset>> {
        // dateparser returns the parsed instant with a UTC offset, so convert it for local display.
        if utc {
            dateparser::parse_with_timezone(s, &Utc).ok().map(|d| d.fixed_offset())
        } else {
            dateparser::parse_with_timezone(s, &Local).ok().map(|d| d.with_timezone(&Local).fixed_offset())
        }
    }

    fn date_from_int(v: i64, utc: bool) -> Option<DateTime<FixedOffset>> {
        if v <= 1e11 as i64 {
            date_from_seconds(v, 0, utc)
        } else if v < 1e14 as i64 {
            date_from_seconds(v.div_euclid(1000), (v.rem_euclid(1000) * 1_000_000) as u32, utc)
        } else {
            None
        }
    }

    fn date_from_float(v: f64, utc: bool) -> Option<DateTime<FixedOffset>> {
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
        date_from_seconds(secs, nanos, utc)
    }

    let now = if opts.utc { Utc::now().fixed_offset() } else { Local::now().fixed_offset() };

    for key in &opts.time_field {
        let v = match obj.get(key) {
            Some(v) => v,
            None => continue,
        };
        let date = match v {
            Value::Number(n) => match n.as_i64() {
                Some(v) => date_from_int(v, opts.utc),
                None => n.as_f64().and_then(|v| date_from_float(v, opts.utc)),
            },
            Value::String(s) => date_from_string(s, opts.utc),
            _ => None,
        };
        let v = match date {
            Some(d) => match &opts.time_format {
                Some(format) => d.format(format).to_string(),
                None if d.date_naive() != now.date_naive() => format!("{}", d.format("%Y-%m-%d %H:%M:%S.%3f")),
                None => format!("{}", d.format("%H:%M:%S.%3f")),
            },
            None => match v {
                Value::Number(n) => n.to_string(),
                Value::String(s) => escape_control_chars(s).into_owned(),
                _ => v.to_string(),
            },
        };
        return (v.color(Color::BrightBlack), key.clone());
    }

    (ColoredString::default(), String::new())
}

fn get_level(obj: &Map<String, Value>, keys: &[String], pad: bool) -> (ColoredString, String) {
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

    for key in keys {
        let v = match obj.get(key) {
            Some(v) => v,
            None => continue,
        };
        let normalized_level = match v {
            Value::Number(n) => normalize_int_log_level(n.as_u64().unwrap_or(0)),
            Value::String(s) => normalize_string_log_level(&escape_control_chars(s)),
            _ => format!("invalid ({})", v),
        };
        return (colorize_log_level(normalized_level.to_uppercase(), pad), key.clone());
    }

    (ColoredString::default(), String::new())
}

fn get_message(obj: &Map<String, Value>, keys: &[String]) -> (Vec<ColoredString>, String) {
    for key in keys {
        let v = match obj.get(key) {
            Some(v) => v,
            None => continue,
        };
        let lines = match v {
            Value::Number(n) => vec![n.to_string().color(Color::White).bold()],
            Value::String(s) => s
                .lines()
                .map(|line| escape_control_chars(line).into_owned().color(Color::White).bold())
                .collect(),
            _ => vec![v.to_string().color(Color::BrightRed).bold()],
        };
        return (lines, key.clone());
    }

    (Vec::new(), String::new())
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

enum RenderedValue {
    Inline(String),
    /// Logical lines without leading indentation; the caller indents the block.
    Block(Vec<String>),
}

fn indent_lines(lines: Vec<String>) -> Vec<String> {
    lines.into_iter().map(|line| format!("  {line}")).collect()
}

fn render_value(path: &str, v: &Value, filter: &FieldFilter) -> RenderedValue {
    match v {
        Value::String(s) if s.contains('\n') => RenderedValue::Block(s.lines().map(|line| escape_control_chars(line).into_owned()).collect()),
        Value::String(s) => RenderedValue::Inline(format!(r#""{}""#, escape_control_chars(s)).color(Color::BrightBlue).to_string()),
        Value::Number(n) => RenderedValue::Inline(format!("{}", n).color(Color::BrightCyan).to_string()),
        Value::Bool(b) => RenderedValue::Inline(format!("{}", b).color(Color::BrightGreen).to_string()),
        Value::Object(map) => render_object(path, map, filter),
        Value::Array(array) => render_array(path, array, filter),
        Value::Null => RenderedValue::Inline("null".color(Color::BrightRed).to_string()),
    }
}

fn render_object(path: &str, map: &Map<String, Value>, filter: &FieldFilter) -> RenderedValue {
    let children: Vec<(Cow<'_, str>, RenderedValue)> = ordered_fields(path, map, filter)
        .into_iter()
        .map(|(key, value, child_path)| (escape_control_chars(key), render_value(&child_path, value, filter)))
        .collect();

    if children.iter().all(|(_, value)| matches!(value, RenderedValue::Inline(_))) {
        let parts: Vec<String> = children
            .into_iter()
            .filter_map(|(key, value)| match value {
                RenderedValue::Inline(value) => Some(format!("{}{}{}", key.color(Color::BrightBlack), ":".color(Color::BrightYellow), value)),
                RenderedValue::Block(_) => None,
            })
            .collect();
        return RenderedValue::Inline(format!(
            "{}{}{}",
            "{".color(Color::BrightYellow),
            parts.join(", ".color(Color::BrightYellow).to_string().as_str()),
            "}".color(Color::BrightYellow)
        ));
    }

    let mut lines = Vec::new();
    for (key, value) in children {
        match value {
            RenderedValue::Inline(value) => lines.push(format!("{}: {}", key.color(Color::BrightBlack), value)),
            RenderedValue::Block(block) => {
                lines.push(format!("{}:", key.color(Color::BrightBlack)));
                lines.extend(indent_lines(block));
            }
        }
    }
    RenderedValue::Block(lines)
}

fn render_array(path: &str, array: &[Value], filter: &FieldFilter) -> RenderedValue {
    let items: Vec<RenderedValue> = array.iter().map(|item| render_value(path, item, filter)).collect();

    if items.iter().all(|item| matches!(item, RenderedValue::Inline(_))) {
        let parts: Vec<String> = items
            .into_iter()
            .filter_map(|item| match item {
                RenderedValue::Inline(value) => Some(value),
                RenderedValue::Block(_) => None,
            })
            .collect();
        return RenderedValue::Inline(format!(
            "{}{}{}",
            "[".color(Color::BrightMagenta),
            parts.join(", ".color(Color::BrightMagenta).to_string().as_str()),
            "]".color(Color::BrightMagenta)
        ));
    }

    let mut lines = Vec::new();
    for item in items {
        match item {
            RenderedValue::Inline(value) => lines.push(format!("{} {}", "-".color(Color::BrightMagenta), value)),
            RenderedValue::Block(block) => {
                let mut block = block.into_iter();
                if let Some(first) = block.next() {
                    lines.push(format!("{} {}", "-".color(Color::BrightMagenta), first));
                    lines.extend(indent_lines(block.collect()));
                }
            }
        }
    }
    RenderedValue::Block(lines)
}

/// Returns the fields that fit on the header line and the multiline fields that follow it.
fn get_fields(
    obj: &Map<String, Value>,
    exclude_fields: &HashSet<String>,
    include_fields: Option<&HashSet<String>>,
    field_order: &[String],
) -> (String, String) {
    enum RenderedField {
        Inline(String),
        Block(String),
    }

    fn get_field(key: &str, v: &Value, path: &str, filter: &FieldFilter) -> RenderedField {
        let key = escape_control_chars(key);
        match render_value(path, v, filter) {
            RenderedValue::Inline(value) => RenderedField::Inline(format!("{}={}", key.color(Color::BrightBlack), value)),
            RenderedValue::Block(lines) => {
                let mut block = vec![format!("{}:", key.color(Color::BrightBlack))];
                block.extend(indent_lines(lines));
                RenderedField::Block(block.join("\n"))
            }
        }
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

    (inline_fields.join(" "), block_fields.join("\n"))
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
            time_field: vec!["time".to_string(), "timestamp".to_string()],
            level_field: vec!["level".to_string(), "lvl".to_string()],
            message_field: vec!["message".to_string(), "msg".to_string()],
            include_fields: None,
            exclude_fields: HashSet::new(),
            field_order: Vec::new(),
            utc: false,
            time_format: None,
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

            let (fractional, _) = get_time(fractional.as_object().unwrap(), &test_options());
            let (whole, _) = get_time(whole.as_object().unwrap(), &test_options());

            assert_eq!(fractional.to_string(), whole.to_string());
        }
    }

    #[test]
    fn get_time_formats_in_utc_with_custom_format() {
        disable_colors();

        let mut opts = test_options();
        opts.utc = true;
        opts.time_format = Some("%Y-%m-%d %H:%M:%S%.3f".to_string());

        let line = r#"{"time":1624829360868,"level":30,"message":"x"}"#;
        assert_eq!(format_line(line, &opts), "2021-06-27 21:29:20.868  INFO x");
    }

    #[test]
    fn time_format_validation_rejects_unknown_specifiers() {
        assert!(is_valid_time_format("%Y-%m-%d %H:%M:%S%.3f"));
        assert!(!is_valid_time_format("%Q"));
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
        let (inline, block) = get_fields(
            obj.as_object().unwrap(),
            &["time".to_string(), "level".to_string(), "message".to_string()].iter().cloned().collect(),
            None,
            &[],
        );

        assert!(inline.contains("request_id=42"));
        assert!(inline.contains("ok=true"));
        assert!(block.is_empty());
    }

    #[test]
    fn format_line_renders_nested_multiline_fields_as_blocks() {
        disable_colors();

        let line = r#"{"level":50,"message":"boom","err":{"code":"DB_TIMEOUT","stack":"Error: boom\n    at main"}}"#;
        let formatted = format_line(line, &test_options());

        assert_eq!(
            formatted,
            r#"ERROR boom
err:
  code: "DB_TIMEOUT"
  stack:
    Error: boom
        at main"#
        );
    }

    #[test]
    fn format_line_renders_multiline_messages_as_continuation() {
        disable_colors();

        let line = r#"{"level":30,"message":"first\nsecond","a":1}"#;
        let formatted = format_line(line, &test_options());

        assert_eq!(formatted, "INFO first a=1\n  second");
    }

    #[test]
    fn format_line_renders_multiline_array_items_as_blocks() {
        disable_colors();

        let line = r#"{"level":30,"message":"x","items":["a\nb","c"]}"#;
        let formatted = format_line(line, &test_options());

        assert_eq!(
            formatted,
            r#"INFO x
items:
  - a
    b
  - "c""#
        );
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
