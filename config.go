package main

// Config is a struct that contains configuration for the server.
type Config struct {
	// Address is where the server will be available.
	Address string

	Root string

	// Users is used for authentication purposes.
	Users map[string]string
}
