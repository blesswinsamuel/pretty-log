package cmd

import (
	"log"
	"os"

	"github.com/blesswinsamuel/pretty-json-log/internal"
	"github.com/spf13/cobra"
)

var (
	prettyJsonLogConfig internal.PrettyJsonLogConfig

	rootCmd = &cobra.Command{
		Use:   "pretty-json-log",
		Short: "Pretty JSON Log parses JSON logs passed via stdin and shows it in easily readable format with colors",
		Long:  ``,
		Run: func(cmd *cobra.Command, args []string) {
			pl := internal.NewPrettyJsonLog(prettyJsonLogConfig)
			pl.Run()
		},
	}
)

func init() {
	cobra.OnInitialize(initConfig)

	rootCmd.Flags().StringVar(&prettyJsonLogConfig.TimeFieldKey, "time-field", "time,timestamp", "field that represents time")
	rootCmd.Flags().StringVar(&prettyJsonLogConfig.LevelFieldKey, "level-field", "level,lvl", "field that represents log level")
	rootCmd.Flags().StringVar(&prettyJsonLogConfig.MessageFieldKey, "message-field", "message,msg", "field that represents message")
	rootCmd.Flags().StringVar(&prettyJsonLogConfig.OutputTimeFmt, "time-format", "{t}{ms}", "time format (eg. '{d} {t}{ms}')")
}

func initConfig() {
}

func Execute() {
	if err := rootCmd.Execute(); err != nil {
		log.Println(err)
		os.Exit(1)
	}
}
