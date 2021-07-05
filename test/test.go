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
		v, _ := json.Marshal(map[string]interface{}{
			"level":   levels[i%len(levels)],
			"time":    time.Now().Unix(),
			"message": "this is a test message",
			"object":  map[string]interface{}{"string": "hey there", "int": 20, "float": 1.2, "bool": true, "null": nil, "object": map[string]interface{}{"string": "hey there", "int": 20, "float": 1.2, "bool": true, "null": nil}},
			"array":   []interface{}{"string", 1, true, 1.2, nil},
			"string":  "hey there %s",
			"int":     20,
			"float":   1.2,
			"bool":    true,
			"null":    nil,
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
	fmt.Println(`{"message": "stopped"}`)
	time.Sleep(500 * time.Millisecond)
	fmt.Println(`{"message": "stopped again"}`)
}
