# pretty-json-log

pretty-json-log parses JSON logs and shows them in a pretty format with colors easier to read.

## Install

```
go get -u github.com/blesswinsamuel/pretty-json-log
```

## Usage

```
./your-application | pretty-json-log
```

Use `pretty-json-log --help` for usage information.

## Development

```
go run test/test.go | go run .
# or
go install && go run test/test.go | pretty-json-log
```
