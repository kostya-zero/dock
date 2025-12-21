package main

import (
	"fmt"
	"os"
)

func main() {
	cli := BuildCli()
	if err := cli.Execute(); err != nil {
		fmt.Fprintf(os.Stderr, "An error occured: %e\n", err)
		os.Exit(1)
	}
}
