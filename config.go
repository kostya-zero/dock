package main

import (
	"encoding/json"
	"os"
)

// Config is a struct that contains configuration for the server.
type Config struct {
	// Address is where the server will be available.
	Address string `json:"address"`

	// Volumes allows to separate multiple locations.
	// Volumes map[string]string `json:"volumes"`
	Root string

	// Users is used for authentication purposes.
	Users map[string]string `json:"users"`
}

func LoadConfig(path string) (*Config, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	var cfg Config
	if err := json.Unmarshal(data, &cfg); err != nil {
		return nil, err
	}

	return &cfg, nil
}
