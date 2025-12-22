package main

import (
	"errors"
	"fmt"
	"io"
	"net"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	"github.com/charmbracelet/log"
)

type Session struct {
	User          string
	cwd           string
	ActiveAddr    *net.TCPAddr
	authed        bool
	conn          net.Conn
	server        *Server
	restOffset    int64
	logger        *log.Logger
	pasvListeneer net.Listener
}

func (s *Session) reply(code int, msg string) {
	fmt.Fprintf(s.conn, "%d %s\r\n", code, msg)
}

func (s *Session) handle(cmd, arg string) error {
	l.Info("Received a new command", "command", cmd, "arg", arg)

	switch cmd {
	case "OPTS":
		if arg == "" {
			s.reply(501, "Arguments is empty.")
			return nil
		}

		splitted := strings.SplitN(arg, " ", 2)
		option := splitted[0]
		switch option {
		case "UTF8":
			s.reply(200, "UTF-8 enabled by default.")
		}
		return nil
	case "USER":
		if err := s.server.checkUser(arg); err != nil {
			s.reply(530, "User denied.")
			return nil
		}

		s.User = arg
		s.reply(331, "Password is required.")
	case "PORT":
		if !s.authed {
			s.reply(530, "Login required.")
			return nil
		}
		return s.cmdPort(arg)
	case "PASS":
		if s.User == "" {
			s.reply(530, "Username not provided")
			return nil
		}

		if err := s.server.checkPassword(s.User, arg); err != nil {
			s.reply(530, "wrong password")
			return nil
		}

		s.authed = true
		s.reply(230, "Login success.")
	case "FEAT":
		features := [4]string{"UTF8", "MLST type*;size*;modify*;perm*;", "PASV", "PORT"}
		s.reply(211, "Features")
		for _, str := range features {
			fmt.Fprintf(s.conn, " %s\r\n", str)
		}
		s.reply(211, "End")
	case "SYST":
		s.reply(215, "UNIX Type: L8")
	case "PASV":
		if !s.authed {
			s.reply(530, "Login required.")
			return nil
		}
		return s.cmdPasv()
	case "TYPE":
		s.reply(200, "OK")
	case "LIST", "NLST", "MLST", "MLSD":
		if !s.authed {
			s.reply(530, "Login required.")
			return nil
		}
		return s.cmdList(arg)
	case "PWD", "XPWD":
		s.reply(257, fmt.Sprintf("\"%s\" is the current directory.", s.cwd))
	case "CWD":
		if !s.authed {
			s.reply(530, "Login required.")
			return nil
		}
		if arg == "" {
			return errors.New("CWD requires path")
		}
		p, err := s.absPath(arg)
		if err != nil {
			return err
		}
		fi, err := os.Stat(p)
		if err != nil || !fi.IsDir() {
			return errors.New("not a directory")
		}

		s.cwd = cleanFtpPath(joinFtp(s.cwd, arg))
		s.reply(250, "Directory changed.")
		return nil
	case "RETR":
		if !s.authed {
			s.reply(530, "Login required.")
			return nil
		}
		return s.cmdRetr(arg)
	case "REST":
		if !s.authed {
			s.reply(530, "Login required.")
			return nil
		}
		if arg == "" {
			return errors.New("number is required")
		}
		offset, err := strconv.Atoi(arg)
		if err != nil {
			return errors.New("not a number")
		}
		s.restOffset = int64(offset)
		s.reply(350, "Restarting at specified bytes.")
		return nil
	case "SIZE":
		if !s.authed {
			s.reply(530, "Login required.")
			return nil
		}
		if arg == "" {
			return errors.New("SIZE requires path")
		}
		p, err := s.absPath(arg)
		if err != nil {
			return err
		}
		fi, err := os.Stat(p)
		if err != nil || fi.IsDir() {
			return errors.New("not a file")
		}

		s.reply(213, fmt.Sprintf("%d", fi.Size()))

		return nil
	case "QUIT":
		s.reply(221, "Bye!")
		_ = s.conn.Close()
		return nil
	}

	return nil
}

func (s *Session) openDataConnection() (net.Conn, error) {
	// For Active mode
	if s.ActiveAddr != nil {
		d := net.Dialer{Timeout: 10 * time.Second}
		c, err := d.Dial("tcp", s.ActiveAddr.String())
		if err != nil {
			return nil, err
		}
		s.ActiveAddr = nil
		return c, nil
	}

	// For passive mode
	if s.pasvListeneer == nil {
		return nil, errors.New("use PASV or PORT first")
	}

	type acceptRes struct {
		c   net.Conn
		err error
	}
	ch := make(chan acceptRes, 1)
	go func() {
		c, err := s.pasvListeneer.Accept()
		ch <- acceptRes{c: c, err: err}
	}()

	select {
	case res := <-ch:
		_ = s.pasvListeneer.Close()
		s.pasvListeneer = nil
		return res.c, res.err
	case <-time.After(10 * time.Second):
		_ = s.pasvListeneer.Close()
		s.pasvListeneer = nil
		return nil, errors.New("data connection timeout")
	}
}

func (s *Session) cmdRetr(arg string) error {
	if arg == "" {
		return errors.New("RETR requires filename")
	}

	p, err := s.absPath(joinFtp(s.cwd, arg))
	if err != nil {
		return err
	}

	f, err := os.Open(p)
	if err != nil {
		return err
	}
	defer f.Close()

	info, _ := f.Stat()

	if s.restOffset > 0 {
		if s.restOffset >= info.Size() {
			s.reply(550, "Invalid restart position.")
			s.restOffset = 0
			return nil
		}
		_, _ = f.Seek(s.restOffset, io.SeekStart)
	}

	data, err := s.openDataConnection()
	if err != nil {
		return err
	}
	defer data.Close()

	s.reply(150, "Beginning transfer...")
	_, err = io.Copy(data, f)
	if err != nil {
		return err
	}
	s.reply(226, "Transfer finished")
	return nil
}

func (s *Session) cmdList(arg string) error {
	data, err := s.openDataConnection()
	if err != nil {
		return err
	}
	defer data.Close()

	s.reply(150, "Listing of directory")

	dirPath := s.cwd
	if strings.TrimSpace(arg) != "" {
		dirPath = cleanFtpPath(joinFtp(s.cwd, arg))
	}

	p, err := s.absPath(dirPath)
	if err != nil {
		return err
	}

	entries, err := os.ReadDir(p)
	if err != nil {
		return err
	}

	links := "1"
	owner := "root"
	group := "group"

	for _, e := range entries {
		name := e.Name()
		info, err := e.Info()
		if err != nil {
			return err
		}

		mode := info.Mode().String()
		size := strconv.Itoa(int(info.Size()))

		timestamp := info.ModTime().In(time.UTC)
		timestampStr := timestamp.Format("Jan 02 15:04")

		fmt.Fprintf(data, "%s %s %s %s %s %s %s\r\n", mode, links, owner, group, size, timestampStr, name)
	}

	s.reply(226, "Listing done.")

	return nil
}

func (s *Session) cmdPasv() error {
	if s.pasvListeneer != nil {
		_ = s.pasvListeneer.Close()
		s.pasvListeneer = nil
	}

	ln, err := net.Listen("tcp", "0.0.0.0:0")
	if err != nil {
		return err
	}
	s.pasvListeneer = ln

	addr := ln.Addr().(*net.TCPAddr)
	hostIP, ok := s.conn.LocalAddr().(*net.TCPAddr)
	ipStr := ""
	if ok && hostIP.IP != nil && hostIP.IP.To4() != nil && !hostIP.IP.IsUnspecified() {
		ipStr = hostIP.IP.String()
	} else {
		ipStr = "127.0.0.1"
	}

	p1 := addr.Port / 256
	p2 := addr.Port % 256

	h := strings.Split(ipStr, ".")
	s.reply(227, fmt.Sprintf("Entering Passive Mode (%s,%s,%s,%s,%d,%d)", h[0], h[1], h[2], h[3], p1, p2))

	return nil
}

func (s *Session) cmdPort(arg string) error {
	parts := strings.Split(arg, ",")
	if len(parts) != 6 {
		s.reply(501, "Syntax error in arguments.")
		return nil
	}

	h1 := strings.TrimSpace(parts[0])
	h2 := strings.TrimSpace(parts[1])
	h3 := strings.TrimSpace(parts[2])
	h4 := strings.TrimSpace(parts[3])

	p1, err := strconv.Atoi(parts[4])
	if err != nil || p1 < 0 || p1 > 255 {
		s.reply(501, "Invalid port.")
		return nil
	}

	p2, err := strconv.Atoi(parts[5])
	if err != nil || p2 < 0 || p2 > 255 {
		s.reply(501, "Invalid port.")
		return nil
	}

	ipString := fmt.Sprintf("%s.%s.%s.%s", h1, h2, h3, h4)
	ip := net.ParseIP(ipString)
	if ip == nil || ip.To4() == nil {
		s.reply(501, "Invalid IP address.")
		return nil
	}

	port := p1*256 + p2

	if s.pasvListeneer != nil {
		_ = s.pasvListeneer.Close()
		s.pasvListeneer = nil
	}

	s.ActiveAddr = &net.TCPAddr{IP: ip.To4(), Port: port}
	s.reply(200, "PORT command success.")
	return nil
}

func (s *Session) absPath(ftpPath string) (string, error) {
	full := ftpPath
	if !strings.HasPrefix(full, "/") {
		full = joinFtp(s.cwd, full)
	}
	full = cleanFtpPath(full)

	osPath := filepath.Join(s.server.Root, filepath.FromSlash(strings.TrimPrefix(full, "/")))
	osPath = filepath.Clean(osPath)

	rootClean := filepath.Clean(s.server.Root)
	rel, err := filepath.Rel(rootClean, osPath)
	if err != nil {
		return "", err
	}
	if rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return "", errors.New("access denied")
	}
	return osPath, nil
}

func joinFtp(base, add string) string {
	if strings.HasPrefix(add, "/") {
		return add
	}
	if base == "/" {
		return "/" + add
	}
	return base + "/" + add
}

func cleanFtpPath(p string) string {
	p = strings.ReplaceAll(p, "\\", "/")
	p = filepath.ToSlash(filepath.Clean(p))
	if !strings.HasPrefix(p, "/") {
		p = "/" + p
	}
	if p == "." {
		p = "/"
	}
	return p
}
