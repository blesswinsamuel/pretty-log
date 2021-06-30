package internal

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

	"github.com/araddon/dateparse"
	"github.com/fatih/color"
)

type PrettyJsonLogConfig struct {
	TimeFieldKey    string
	LevelFieldKey   string
	MessageFieldKey string
	OutputTimeFmt   string
}

type PrettyJsonLog struct {
	config PrettyJsonLogConfig
}

func NewPrettyJsonLog(config PrettyJsonLogConfig) *PrettyJsonLog {
	return &PrettyJsonLog{config}
}

func (p *PrettyJsonLog) Run() {
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
		p.printLogs(ch)
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

func (p *PrettyJsonLog) printLogs(ch <-chan string) {
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

	dateFormatReplacer := strings.NewReplacer("{d}", "2006-01-02", "{t}", "15:04:05", "{ms}", ".000")
	displayTimeFormat := dateFormatReplacer.Replace(p.config.OutputTimeFmt)

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

	getTime := func() string {
		ti := getInterfaceField(p.config.TimeFieldKey, "")
		tstr := ""
		switch v := ti.(type) {
		case string:
			tstr = v
		case float64:
			tstr = fmt.Sprint(int64(v))
		case int:
			tstr = fmt.Sprint(v)
		}
		if tstr == "" {
			return timeColor.Sprint("EMPTY TIME")
		}

		tp, err := dateparse.ParseAny(tstr)
		if err != nil {
			return timeColor.Sprintf("INVALID TIME [%v]", err)
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
		level := normalizeLogLevel(getInterfaceField(p.config.LevelFieldKey, "unknown"))
		c, ok := logColors[level]
		if !ok {
			return logColors["DEFAULT"].Sprint(level)
		}
		return c.Sprintf("%5s", level)
	}

	getMessage := func() string {
		return messageColor.Sprint(getStringField(p.config.MessageFieldKey, color.New(color.FgHiRed).Sprint("null")))
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
		delete(line, p.config.LevelFieldKey)
		time := getTime()
		delete(line, p.config.TimeFieldKey)
		message := getMessage()
		delete(line, p.config.MessageFieldKey)
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
	for k := range m {
		res = append(res, k)
	}
	sort.Strings(res)
	return res
}
