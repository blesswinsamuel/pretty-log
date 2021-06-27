package cmd

import (
	"bufio"
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"os"
	"os/signal"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/fatih/color"
	"github.com/spf13/cobra"
)

var (
	timeFieldKey    string
	levelFieldKey   string
	messageFieldKey string
	timeFormat      string
	showDate        bool
	showMillis      bool

	rootCmd = &cobra.Command{
		Use:   "pretty-json-log",
		Short: "Pretty JSON Log parses JSON logs passed via stdin and shows it in easily readable format with colors",
		Long:  ``,
		Run: func(cmd *cobra.Command, args []string) {
			prettyJsonLog()
		},
	}
)

func init() {
	cobra.OnInitialize(initConfig)

	rootCmd.Flags().StringVar(&timeFieldKey, "time-field", "time", "field that represents time")
	rootCmd.Flags().StringVar(&timeFormat, "time-format", "unix-s", "format in which time is represented")
	rootCmd.Flags().StringVar(&levelFieldKey, "level-field", "level", "field that represents log level")
	rootCmd.Flags().StringVar(&messageFieldKey, "message-field", "message", "field that represents message")
	rootCmd.Flags().BoolVar(&showDate, "show-date", false, "show date")
	rootCmd.Flags().BoolVar(&showMillis, "show-millis", true, "show millis")
}

func initConfig() {
}

func Execute() {
	if err := rootCmd.Execute(); err != nil {
		log.Println(err)
		os.Exit(1)
	}
}

func prettyJsonLog() {
	stopCh := make(chan os.Signal, 1)
	ch := make(chan string, 10)

	wgRead := sync.WaitGroup{}
	for _, stream := range []io.Reader{os.Stdin} { // os.Stderr, os.Stdout
		wgRead.Add(1)
		go func(stream io.Reader) {
			readLogs(stream, ch)
			close(stopCh)
			wgRead.Done()
		}(stream)
	}

	wgPrint := sync.WaitGroup{}
	wgPrint.Add(1)
	go func() {
		defer wgPrint.Done()
		printLogs(ch)
	}()

	signal.Notify(stopCh, os.Interrupt)

	<-stopCh
	wgRead.Wait()
	close(ch)
	wgPrint.Wait()
}

func readLogs(reader io.Reader, ch chan<- string) {
	scanner := bufio.NewScanner(os.Stdin)

	for scanner.Scan() {
		text := scanner.Text()
		if strings.TrimSpace(text) != "" {
			ch <- text
		}

		if err := scanner.Err(); err != nil {
			log.Println(err)
		}
	}
}

func printLogs(ch <-chan string) {
	var line map[string]json.RawMessage

	getInterfaceField := func(key string, def interface{}) interface{} {
		vraw, ok := line[key]
		if !ok {
			return def
		}
		var vi interface{}
		if err := json.Unmarshal(vraw, &vi); err != nil {
			return def
		}
		// fmt.Println(fmt.Sprintf("%T", vi))
		return vi
	}

	getStringField := func(key string, def string) string {
		vi := getInterfaceField(key, def)
		v, ok := vi.(string)
		if !ok {
			return def // fmt.Sprint(vi)
		}
		return v
	}
	getIntField := func(key string, def int64) int64 {
		v, ok := getInterfaceField(key, def).(float64)
		if !ok {
			return def
		}
		return int64(v)
	}

	displayTimeFormat := "15:04:05"
	if showDate {
		displayTimeFormat = "2006-01-02 " + displayTimeFormat
	}
	if showMillis && timeFormat != "unix-s" {
		displayTimeFormat = displayTimeFormat + ".000"
	}

	timeColor := color.New(color.FgHiBlack, color.Bold)
	messageColor := color.New(color.FgHiWhite, color.Bold)

	fieldKeyColor := color.New(color.FgHiBlack)

	logColors := map[string]*color.Color{
		"PANIC": color.New(color.FgRed, color.Bold, color.BgHiWhite),
		"FATAL": color.New(color.FgHiWhite, color.Bold, color.BgRed),
		"ERROR": color.New(color.FgHiWhite, color.Bold, color.BgHiRed),
		"WARN":  color.New(color.FgHiBlack, color.Bold, color.BgHiYellow),
		"INFO":  color.New(color.FgHiWhite, color.Bold, color.BgHiBlue),
		"DEBUG": color.New(color.FgHiWhite, color.Bold, color.BgHiBlack),
		"TRACE": color.New(color.FgHiWhite, color.Bold, color.BgBlack),

		"DEFAULT": color.New(color.FgWhite).Add(color.Bold).Add(color.BgHiBlack),
	}

	timeFormats := map[string]string{
		"rfc-3339-nano": time.RFC3339Nano,
		"iso-8601":      "2006-01-02T15:04:05-0700", // https://stackoverflow.com/a/38596248
	}

	getTime := func() string {
		switch timeFormat {
		case "unix-s":
			ti := getIntField(timeFieldKey, 0)
			if ti == 0 {
				return timeColor.Sprint("INVALID TIME")
			}
			return timeColor.Sprint(time.Unix(ti, 0).Local().Format(displayTimeFormat))
		case "unix-ms":
			ti := getIntField(timeFieldKey, 0)
			if ti == 0 {
				return timeColor.Sprint("INVALID TIME")
			}
			return timeColor.Sprint(time.Unix(0, ti*int64(time.Millisecond)).Local().Format(displayTimeFormat))
		}
		timeFmt, ok := timeFormats[timeFormat]
		if !ok {
			timeFmt = timeFormat
		}
		ti := getStringField(timeFieldKey, "")
		if ti == "" {
			return timeColor.Sprint("INVALID TIME")
		}
		tp, err := time.Parse(timeFmt, ti)
		if err != nil {
			log.Println(err)
			return timeColor.Sprint("INVALID TIME")
		}
		return timeColor.Sprint(tp.Local().Format(displayTimeFormat))
	}

	intLevels := map[int]string{
		10: "trace",
		20: "debug",
		30: "info",
		40: "warn",
		50: "error",
		60: "fatal",
	}

	normalizeLogLevel := func(l interface{}) string {
		switch l := l.(type) {
		case float64:
			level, ok := intLevels[int(l)]
			if !ok {
				return fmt.Sprint(l)
			}
			return strings.ToUpper(level)
		case string:
			return strings.ToUpper(l)
		}
		return fmt.Sprint(l)
	}

	getLevel := func() string {
		level := normalizeLogLevel(getInterfaceField(levelFieldKey, "unknown"))
		c, ok := logColors[level]
		if !ok {
			return logColors["DEFAULT"].Sprint(level)
		}
		return c.Sprintf("%5s", level)
	}

	getMessage := func() string {
		return messageColor.Sprint(getStringField(messageFieldKey, color.New(color.FgHiRed).Sprint("null")))
	}

	getField := func(k string, f json.RawMessage) string {
		var getFieldValue func(vi interface{}) string
		getFieldValue = func(vi interface{}) string {
			switch vi := vi.(type) {
			case string:
				return color.New(color.FgHiBlue).Sprintf(`"%s"`, vi)
			case json.Number:
				return color.New(color.FgHiCyan).Sprint(vi)
			case bool:
				return color.New(color.FgHiGreen).Sprint(vi)
			case map[string]interface{}:
				res := []string{}
				for _, k := range sortedKeys(vi) {
					res = append(res, fmt.Sprintf("%s=%s", fieldKeyColor.Sprint(k), getFieldValue(vi[k])))
				}
				c := color.New(color.FgHiYellow)
				return fmt.Sprintf("%s%s%s", c.Sprint("{"), strings.Join(res, c.Sprint(", ")), c.Sprint("}"))
			case []interface{}:
				res := []string{}
				for _, v := range vi {
					res = append(res, fmt.Sprint(getFieldValue(v)))
				}
				c := color.New(color.FgHiMagenta)
				return fmt.Sprintf("%s%s%s", c.Sprint("["), strings.Join(res, c.Sprint(", ")), c.Sprint("]"))
			case nil:
				return color.New(color.FgHiRed).Sprint("null")
			}
			return color.New(color.FgWhite).Sprint(vi)
		}
		var vi interface{}
		d := json.NewDecoder(bytes.NewReader(f))
		d.UseNumber()
		if err := d.Decode(&vi); err != nil {
			return ""
		}
		return fmt.Sprintf("%s=%s", fieldKeyColor.Sprint(k), getFieldValue(vi))
	}

	for log := range ch {
		line = map[string]json.RawMessage{}
		if err := json.Unmarshal([]byte(log), &line); err != nil {
			fmt.Println(log)
			continue
		}
		level := getLevel()
		delete(line, levelFieldKey)
		time := getTime()
		delete(line, timeFieldKey)
		message := getMessage()
		delete(line, messageFieldKey)
		fields := []string{}
		for k, f := range line {
			fields = append(fields, getField(k, f))
		}
		sort.Strings(fields)
		fmt.Printf("%s %s %s %s\n", time, level, message, strings.Join(fields, " "))
	}
}

func sortedKeys(m map[string]interface{}) []string {
	res := []string{}
	for k, _ := range m {
		res = append(res, k)
	}
	sort.Strings(res)
	return res
}
