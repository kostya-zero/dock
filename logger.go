package main

import (
	"os"
	"time"

	"github.com/charmbracelet/log"
)

func PrepareLogger() *log.Logger {
	return log.NewWithOptions(os.Stdout, log.Options{
		ReportTimestamp: true,
		TimeFormat:      time.RFC1123,
		ReportCaller:    true,
	})
}
