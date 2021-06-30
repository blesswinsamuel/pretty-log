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

	timeColor         *color.Color
	messageColor      *color.Color
	fieldKeyColor     *color.Color
	logColors         map[string]*color.Color
	intLevels         map[int]string
	displayTimeFormat string
}

func NewPrettyJsonLog(config PrettyJsonLogConfig) *PrettyJsonLog {
	dateFormatReplacer := strings.NewReplacer("{d}", "2006-01-02", "{t}", "15:04:05", "{ms}", ".000")

	p := &PrettyJsonLog{
		config:            config,
		displayTimeFormat: dateFormatReplacer.Replace(config.OutputTimeFmt),

		timeColor:     color.New(color.FgHiBlack, color.Bold),
		messageColor:  color.New(color.FgHiWhite, color.Bold),
		fieldKeyColor: color.New(color.FgHiBlack),
		logColors: map[string]*color.Color{
			"PANIC": color.New(color.FgRed, color.Bold, color.BgHiWhite),
			"FATAL": color.New(color.FgHiWhite, color.Bold, color.BgRed),
			"ERROR": color.New(color.FgHiWhite, color.Bold, color.BgHiRed),
			"WARN":  color.New(color.FgHiBlack, color.Bold, color.BgHiYellow),
			"INFO":  color.New(color.FgHiWhite, color.Bold, color.BgHiBlue),
			"DEBUG": color.New(color.FgHiWhite, color.Bold, color.BgHiBlack),
			"TRACE": color.New(color.FgHiWhite, color.Bold, color.BgBlack),

			"DEFAULT": color.New(color.FgWhite).Add(color.Bold).Add(color.BgHiBlack),
		},
		intLevels: map[int]string{
			10: "trace",
			20: "debug",
			30: "info",
			40: "warn",
			50: "error",
			60: "fatal",
		},
	}
	return p
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
	for logLine := range ch {
		line, err := NewLogLine(logLine, p)
		if err != nil {
			log.Println(err)
			continue
		}
		fmt.Printf("%s %s %s %s\n", line.getTime(), line.getLevel(), line.getMessage(), line.getFields())
	}
}

type logLine struct {
	line map[string]json.RawMessage
	p    *PrettyJsonLog
}

func NewLogLine(log string, p *PrettyJsonLog) (*logLine, error) {
	var line map[string]json.RawMessage
	if err := json.Unmarshal([]byte(log), &line); err != nil {
		return nil, err
	}
	return &logLine{line, p}, nil
}

func (l *logLine) getTime() string {
	ti := l.getInterfaceField(l.p.config.TimeFieldKey, "")
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
		return l.p.timeColor.Sprint("EMPTY TIME")
	}

	tp, err := dateparse.ParseAny(tstr)
	if err != nil {
		return l.p.timeColor.Sprintf("INVALID TIME [%v]", err)
	}
	return l.p.timeColor.Sprint(tp.Local().Format(l.p.displayTimeFormat))
}

func (l *logLine) getMessage() string {
	msg := l.getStringField(l.p.config.MessageFieldKey, color.New(color.FgHiRed).Sprint("null"))
	return l.p.messageColor.Sprint(msg)
}

func (l *logLine) getLevel() string {
	normalizeLogLevel := func(lv interface{}) string {
		switch lv := lv.(type) {
		case float64:
			level, ok := l.p.intLevels[int(lv)]
			if !ok {
				return fmt.Sprint(l)
			}
			return strings.ToUpper(level)
		case string:
			return strings.ToUpper(lv)
		}
		return fmt.Sprint(lv)
	}

	level := normalizeLogLevel(l.getInterfaceField(l.p.config.LevelFieldKey, "unknown"))
	c, ok := l.p.logColors[level]
	if !ok {
		return l.p.logColors["DEFAULT"].Sprint(level)
	}
	return c.Sprintf("%5s", level)
}

func (l *logLine) getFields() string {
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
					res = append(res, fmt.Sprintf("%s=%s", l.p.fieldKeyColor.Sprint(k), getFieldValue(vi[k])))
				}
				c := color.New(color.FgHiYellow)
				return fmt.Sprintf("%s%s%s", c.Sprint("{"), strings.Join(res, c.Sprint(", ")), c.Sprint("}"))
			case []interface{}:
				res := []string{}
				for _, v := range vi {
					res = append(res, getFieldValue(v))
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

		return fmt.Sprintf("%s=%s", l.p.fieldKeyColor.Sprint(k), getFieldValue(vi))
	}
	delete(l.line, l.p.config.LevelFieldKey)
	delete(l.line, l.p.config.TimeFieldKey)
	delete(l.line, l.p.config.MessageFieldKey)
	fields := []string{}
	for k, f := range l.line {
		fields = append(fields, getField(k, f))
	}
	sort.Strings(fields)
	return strings.Join(fields, " ")
}

func (l *logLine) getInterfaceField(key string, def interface{}) interface{} {
	vraw, ok := l.line[key]
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

func (l *logLine) getStringField(key string, def string) string {
	vi := l.getInterfaceField(key, def)
	v, ok := vi.(string)
	if !ok {
		return def // fmt.Sprint(vi)
	}
	return v
}

func sortedKeys(m map[string]interface{}) []string {
	res := []string{}
	for k := range m {
		res = append(res, k)
	}
	sort.Strings(res)
	return res
}
