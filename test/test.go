package main

import (
	"encoding/json"
	"fmt"
	"log"
	"os"
	"os/signal"
	"sync"
	"time"
)

func main() {
	t := time.NewTicker(500 * time.Millisecond)
	quitCh := make(chan struct{})
	wg := sync.WaitGroup{}

	levels := []string{"error", "warn", "info", "debug", "trace"}

	wg.Add(1)
	go func() {
		defer wg.Done()
		i := 0
		for {
			select {
			case <-quitCh:
				return
			case <-t.C:
			}
			v, err := json.Marshal(map[string]interface{}{
				"level":   levels[i%len(levels)],
				"time":    time.Now().Unix(),
				"message": "this is a test message",
				"object":  map[string]interface{}{"string": "hey there", "int": 20, "float": 1.2, "bool": true},
				"string":  "hey there %s",
				"int":     20,
				"float":   1.2,
				"bool":    true,
			})
			if err != nil {
				log.Println(err)
				continue
			}
			fmt.Println(string(v))
			i++
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
