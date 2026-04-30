# pretty-log

pretty-log is a command-line utility that turns newline-delimited JSON logs into compact, readable, colorized output for local debugging.

It is designed to work well with mixed streams where some lines are JSON and some are plain text.

## Quick Preview

Before:

<img width="1228" height="719" alt="image" src="https://github.com/user-attachments/assets/8ebd0c03-348d-4667-a771-2219dfd436df" />

After:

<img width="1228" height="870" alt="image" src="https://github.com/user-attachments/assets/b4126194-beb0-4e61-a74f-44bbe6f339d5" />

## Features

- Human-friendly formatting for structured JSON logs
- Automatic handling for string and numeric log levels (including Pino's `10-60` scale)
- Configurable field aliases for time, level, and message
- Optional include/exclude controls for non-core fields
- Optional field ordering for extra context
- Readable multiline stack trace rendering
- Safe passthrough for non-JSON lines and non-object JSON values

## Installation

### From GitHub

```sh
cargo install --git https://github.com/blesswinsamuel/pretty-log --branch main
```

### From source

```sh
git clone https://github.com/blesswinsamuel/pretty-log.git
cd pretty-log
cargo install --path .
```

## Quick Start

```sh
./your-application | pretty-log
```

Try with included fixtures:

```sh
cat test/logs.txt | cargo run --quiet
cat test/logs_pino.txt | cargo run --quiet -- --color never
```

## CLI Options

```text
Usage: pretty-log [OPTIONS]

Options:
	-t, --time-field <TIME_FIELD>        Field that represents time [default: time,timestamp]
	-l, --level-field <LEVEL_FIELD>      Field that represents level [default: level,lvl]
	-m, --message-field <MESSAGE_FIELD>  Field that represents message [default: message,msg]
			--include-fields <INCLUDE_FIELDS>
																			 Comma-separated non-core fields to include in output
			--exclude-fields <EXCLUDE_FIELDS>
																			 Comma-separated non-core fields to hide from output
			--field-order <FIELD_ORDER>      Comma-separated preferred order for non-core fields
			--color <COLOR>                  Color mode: auto, always, never [default: auto]
	-h, --help                           Print help
	-V, --version                        Print version
```

## Examples

Hide noisy host/process fields:

```sh
cat test/logs_pino.txt | pretty-log --exclude-fields pid,hostname
```

Show only selected context fields:

```sh
cat test/logs_pino.txt | pretty-log --include-fields service,jobId,invoiceId --field-order service,jobId,invoiceId
```

Force plain output (no ANSI colors):

```sh
cat test/logs_pino.txt | pretty-log --color never
```

## Behavior

- Invalid JSON lines are printed unchanged
- Valid non-object JSON values are printed unchanged
- Time/level/message fields are promoted to the log prefix
- Remaining fields are rendered as `key=value` pairs
- Multiline strings (for example stack traces) are rendered as indented blocks

## Development

Run local stream demo:

```sh
go run test/test.go | cargo run
```

Run fixture tests:

```sh
task test
```

Run full validation:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Releases

Release binaries are published via GitHub Actions on tag/release workflows.

## Contributing

Contributions are welcome.

1. Fork the repository
2. Create a feature branch
3. Add or update tests for your change
4. Run validation locally
5. Open a pull request with a clear description

## License

This project is licensed under the terms of the MIT License. See `LICENSE` for details.
