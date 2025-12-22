package main

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"
)

func BuildCli() *cobra.Command {
	var config string
	rootCmd := &cobra.Command{
		Use:   "dock",
		Short: "A port for your files.",
		Run: func(cmd *cobra.Command, args []string) {
			cfg, err := LoadConfig(config)
			if err != nil {
				fmt.Printf("Failed to load configuration: %e", err)
				os.Exit(1)
			}
			StartServer(cfg)
		},
	}
	rootCmd.Flags().StringVarP(&config, "config", "c", "config.json", "path to the configuration file")

	return rootCmd
}
