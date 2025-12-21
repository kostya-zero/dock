package main

import (
	"github.com/spf13/cobra"
)

func BuildCli() *cobra.Command {
	rootCmd := &cobra.Command{
		Use:   "dock",
		Short: "A port for your files.",
		Run: func(cmd *cobra.Command, args []string) {
			m := make(map[string]string)
			m["kostya"] = "123"
			cfg := Config{
				Address: ":21",
				Root:    "D:\\Anime",
				Users:   m,
			}
			StartServer(&cfg)
		},
	}

	return rootCmd
}
