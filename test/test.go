package main

import (
	"encoding/json"
	"fmt"
	"os"
	"os/signal"
	"sync"
	"time"
)

func main() {
	t := time.NewTicker(500 * time.Millisecond)
	quitCh := make(chan struct{})
	wg := sync.WaitGroup{}

	levels := []string{"panic", "fatal", "error", "warn", "info", "debug", "trace"}

	wg.Add(1)

	i := 0

	generateLog := func() string {
		if i%10 == 0 {
			fmt.Println("Plain log line")
		}
		formats := []any{
			time.Now().Unix(),
			time.Now().Unix() * 1000,
			time.Now().Format(time.RFC1123),
			time.Now().Format(time.RFC1123Z),
			time.Now().Format(time.RFC3339),
			time.Now().Format(time.RFC3339Nano),
			time.Now().Format(time.RFC822),
			time.Now().Format(time.RFC822Z),
			time.Now().Format(time.RFC850),
			time.Now().Format(time.RubyDate),
			time.Now().Format(time.UnixDate),
			time.Now().Format(time.ANSIC),
			time.Now().Format(time.Kitchen),
			time.Now().Format(time.Stamp),
			time.Now().Format(time.StampMilli),
			time.Now().Format(time.StampMicro),
			time.Now().Format(time.StampNano),
			time.Now().Format(time.DateTime),
			time.Now().Format(time.DateOnly),
			time.Now().Format(time.TimeOnly),
		}
		t := formats[i%len(formats)]
		v, _ := json.Marshal(map[string]interface{}{
			"level":      levels[i%len(levels)],
			"time":       t,
			"message":    "this is a test message",
			"object":     map[string]interface{}{"string": "hey there", "int": 20, "float": 1.2, "bool": true, "null": nil, "object": map[string]interface{}{"string": "hey there", "int": 20, "float": 1.2, "bool": true, "null": nil}},
			"array":      []interface{}{"string", 1, true, 1.2, nil},
			"actualtime": t,
			"string":     "hey there %s",
			"int":        20,
			"float":      1.2,
			"bool":       true,
			"null":       nil,
		})
		i++
		return string(v)
	}

	go func() {
		defer wg.Done()
		for {
			select {
			case <-quitCh:
				return
			case <-t.C:
			}
			fmt.Println(generateLog())
		}
	}()

	stopCh := make(chan os.Signal, 1)
	signal.Notify(stopCh, os.Interrupt)

	<-stopCh
	close(quitCh)
	wg.Wait()
	fmt.Println(`{"message": "received signal, waiting for graceful shutdown"}`)
	time.Sleep(500 * time.Millisecond)
	fmt.Println(`{"message": "done cleanup, exiting"}`)
}
