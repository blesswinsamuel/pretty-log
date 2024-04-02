# pretty-log

pretty-log parses JSON logs and shows them in a pretty format with colors easier to read.

From this

![From](https://user-images.githubusercontent.com/815723/123560482-01debf80-d7c0-11eb-857a-0f6f830f8822.png)

to

![To](https://user-images.githubusercontent.com/815723/123560502-33f02180-d7c0-11eb-8ba4-dbd50e9ed3d0.png)

## Install

```
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo install --git https://github.com/blesswinsamuel/pretty-log --branch main
```

## Usage

```
./your-application | pretty-log
```

See `pretty-log --help` for usage information.

## Development

```
go run test/test.go | cargo run
# or
cargo install --path . && go run test/test.go | pretty-log
```
