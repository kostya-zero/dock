package main

import (
	"bufio"
	"errors"
	"fmt"
	"net"
	"strings"

	"github.com/charmbracelet/log"
	"github.com/google/uuid"
)

type Server struct {
	Address string
	Root    string
	Users   map[string]string
}

var l *log.Logger

func (s *Server) handleConn(c net.Conn) {
	defer c.Close()

	uuid := uuid.New().String()
	session := &Session{
		ID:               uuid,
		User:             "",
		currentDirectory: "/",
		authorized:       false,
		connection:       c,
		server:           s,
	}

	l.Info("New connection received.", "session", uuid, "ip", c.RemoteAddr().String())

	session.reply(220, "Dock is welcoming you!")

	r := bufio.NewReader(c)
	for {
		line, err := r.ReadString('\n')
		if err != nil {
			return
		}
		line = strings.TrimRight(line, "\r\n")
		if line == "" {
			continue
		}
		cmd, arg := splitCmd(line)
		cmd = strings.ToUpper(cmd)

		if err := session.handle(cmd, arg); err != nil {
			l.Error(fmt.Sprintf("An error occured in handler: %s", err.Error()), "session", uuid)
		}
	}
}

func (s *Server) checkUser(user string) error {
	_, ok := s.Users[user]
	if ok {
		return nil
	} else {
		return errors.New("user not found")
	}
}

func (s *Server) checkPassword(user, pass string) error {
	userPass, ok := s.Users[user]
	if !ok {
		return errors.New("user not found")
	}

	if userPass != pass {
		return errors.New("wrong password")
	}

	return nil
}

func StartServer(c *Config) error {
	l = PrepareLogger()
	l.Infof("Dock FTP Server %s", version)
	l.Info("Starting...")
	listener, err := net.Listen("tcp", c.Address)
	if err != nil {
		return err
	}
	l.Infof("FTP server is listening on %s", c.Address)

	server := &Server{
		Address: c.Address,
		Root:    c.Root,
		Users:   c.Users,
	}

	for {
		c, err := listener.Accept()
		if err != nil {
			l.Errorf("An error occured while accepting connection: %e", err)
			continue
		}

		go server.handleConn(c)
	}
}

func splitCmd(line string) (cmd, arg string) {
	parts := strings.SplitN(line, " ", 2)
	cmd = parts[0]
	if len(parts) == 2 {
		arg = strings.TrimSpace(parts[1])
	}
	return
}
