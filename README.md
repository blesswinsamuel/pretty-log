# pretty-log

pretty-log parses JSON logs and shows them in a pretty format with colors easier to read.

From this

![From](https://user-images.githubusercontent.com/815723/123560482-01debf80-d7c0-11eb-857a-0f6f830f8822.png)

to

![To](https://user-images.githubusercontent.com/815723/123560502-33f02180-d7c0-11eb-8ba4-dbd50e9ed3d0.png)

## Install

```
cargo install --git https://github.com/blesswinsamuel/pretty-json-log --branch main
```

## What It Does

- Formats common JSON log streams into a compact human-readable layout.
- Preserves plain-text lines and non-object JSON values instead of dropping them.
- Supports common timestamp fields and numeric log levels such as Pino's `10-60` scale.
- Renders multiline values like stack traces as indented blocks.
- Lets you control colors and choose which extra fields are shown.

## Usage

```
./your-application | pretty-log
```

Examples:

```sh
cat test/logs.txt | pretty-log
cat test/logs_pino.txt | pretty-log --color never
./your-application | pretty-log --exclude-fields pid,hostname
./your-application | pretty-log --include-fields request_id,service --field-order service,request_id
```

Important flags:

- `--time-field`: comma-separated aliases for the timestamp field. Default: `time,timestamp`
- `--level-field`: comma-separated aliases for the log level field. Default: `level,lvl`
- `--message-field`: comma-separated aliases for the message field. Default: `message,msg`
- `--include-fields`: only show these non-core fields in the formatted suffix
- `--exclude-fields`: hide these non-core fields from the formatted suffix
- `--field-order`: preferred order for non-core fields that are shown
- `--color auto|always|never`: control ANSI color output explicitly

See `pretty-log --help` for the full CLI reference.

## Behavior Notes

- Invalid JSON lines are passed through unchanged.
- Valid JSON values that are not objects are passed through unchanged.
- Object fields used for time, level, and message are promoted into the main prefix and omitted from the extra field list.
- Remaining object fields keep their original order unless `--field-order` is provided.
- Multiline strings are rendered as blocks below the main log line.

## Development

```sh
go run test/test.go | cargo run
task test
```

Validation:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```
