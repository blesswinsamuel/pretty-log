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
- Automatic handling for string and numeric log levels (including common synonyms and Pino's `10-60` scale)
- Configurable field aliases for time, level, and message
- Optional include/exclude controls for non-core fields, including nested `req.id` paths
- Optional field ordering for extra context
- Local or UTC timestamps with optional custom formatting
- Readable multiline rendering for stack traces and messages, at any depth
- Control characters in log content are escaped, so they cannot drive the terminal
- Safe passthrough for non-JSON lines and non-object JSON values

## Installation

### Homebrew

```sh
brew install blesswinsamuel/tap/pretty-log
```

### Prebuilt binaries

macOS and Linux:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/blesswinsamuel/pretty-log/releases/latest/download/pretty-log-installer.sh | sh
```

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
Parses JSON logs and prints them in a clean, colorized view for humans.

Usage: pretty-log [OPTIONS]

Options:
  -t, --time-field <TIME_FIELD>
          Field that represents time [default: time,timestamp]
  -l, --level-field <LEVEL_FIELD>
          Field that represents level [default: level,lvl]
  -m, --message-field <MESSAGE_FIELD>
          Field that represents message [default: message,msg]
      --include-fields <INCLUDE_FIELDS>
          Comma-separated non-core fields to include in the output; dotted paths select nested fields [default: ]
      --exclude-fields <EXCLUDE_FIELDS>
          Comma-separated non-core fields to hide from the output; dotted paths hide nested fields [default: ]
      --field-order <FIELD_ORDER>
          Comma-separated preferred order for non-core fields; dotted paths order nested fields [default: ]
      --utc
          Display timestamps in UTC instead of the local timezone
      --time-format <TIME_FORMAT>
          strftime-style format for timestamps, e.g. "%Y-%m-%d %H:%M:%S%.3f"
      --color <COLOR>
          When to use colorized output [default: auto] [possible values: auto, always, never]
  -h, --help
          Print help
  -V, --version
          Print version
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

Field names may be dotted paths, so nested fields can be selected, hidden, or ordered too:

```sh
cat test/logs.txt | pretty-log --include-fields service,req.id --exclude-fields req.method
```

Show timestamps in UTC with a fixed format:

```sh
cat test/logs.txt | pretty-log --utc --time-format "%Y-%m-%d %H:%M:%S%.3f"
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

Releases are built and published by [cargo-dist](https://axodotdev.github.io/cargo-dist/) when a version tag is pushed:

```sh
git tag v0.3.0
git push origin v0.3.0
```

Each release includes prebuilt binaries for Linux and macOS, SHA-256 checksums, a shell installer, and a Homebrew formula published to [`blesswinsamuel/homebrew-tap`](https://github.com/blesswinsamuel/homebrew-tap).

## Contributing

Contributions are welcome.

1. Fork the repository
2. Create a feature branch
3. Add or update tests for your change
4. Run validation locally
5. Open a pull request with a clear description

## License

This project is licensed under the terms of the MIT License. See `LICENSE` for details.
