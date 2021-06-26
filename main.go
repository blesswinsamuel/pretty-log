package main

import (
	"bufio"
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
)

const timeFieldKey = "time"
const levelFieldKey = "level"
const messageFieldKey = "message"

const timeFormat = "unix_s" // unix_ms or iso8601

func main() {
	ch := make(chan string, 10)

	wgRead := sync.WaitGroup{}
	for _, stream := range []io.Reader{os.Stdin} { // os.Stderr, os.Stdout
		wgRead.Add(1)
		go func(stream io.Reader) {
			defer wgRead.Done()
			readLogs(stream, ch)
		}(stream)
	}

	wgPrint := sync.WaitGroup{}
	wgPrint.Add(1)
	go func() {
		defer wgPrint.Done()
		printLogs(ch)
	}()

	stopCh := make(chan os.Signal, 1)
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
		v, ok := getInterfaceField(key, def).(string)
		if !ok {
			return def
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

	timeFmt := "15:04:05"

	getTime := func() string {
		switch timeFormat {
		case "unix_s":
			ti := getIntField(timeFieldKey, 0)
			t := time.Unix(ti, 0)
			return t.Local().Format(timeFmt)
		}
		return ""
	}

	timeColor := color.New(color.FgHiBlack, color.Bold)
	messageColor := color.New(color.FgHiWhite, color.Bold)

	fieldKeyColor := color.New(color.FgHiBlack)
	fieldValueColor := color.New(color.FgWhite)

	logColors := map[string]*color.Color{
		"ERROR": color.New(color.FgHiWhite, color.Bold, color.BgRed),
		"WARN":  color.New(color.FgHiBlack, color.Bold, color.BgYellow),
		"INFO":  color.New(color.FgHiWhite, color.Bold, color.BgBlue),
		"DEBUG": color.New(color.FgHiWhite, color.Bold, color.BgHiBlack),
		"TRACE": color.New(color.FgHiWhite, color.Bold, color.BgBlack),

		"DEFAULT": color.New(color.FgWhite).Add(color.Bold).Add(color.BgHiBlack),
	}

	getLevel := func() string {
		level := strings.ToUpper(getStringField(levelFieldKey, "unknown"))
		c, ok := logColors[level]
		if !ok {
			return logColors["DEFAULT"].Sprint(level)
		}
		return c.Sprintf("%5s", level)
	}

	getMessage := func() string {
		message := getStringField(messageFieldKey, "")
		if message == "" {
			return ""
		}
		return messageColor.Sprintf("%s", message)
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
			fields = append(fields, fmt.Sprintf("%s=%s", fieldKeyColor.Sprintf(k), fieldValueColor.Sprintf(string(f))))
		}
		sort.Strings(fields)
		fmt.Printf("%s %s %s %s\n", timeColor.Sprint(time), level, message, strings.Join(fields, " "))
	}
}
