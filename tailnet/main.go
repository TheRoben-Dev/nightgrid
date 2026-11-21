// nightgrid-tailnet: Nightgrid's built-in Tailscale client (M10j).
//
// A small sidecar the app starts when "Built-in Tailscale" is on. It joins
// the user's tailnet as its own node (userspace networking via tsnet, no
// system VPN, no admin rights) and offers one thing to the app: a SOCKS5
// proxy on 127.0.0.1 with a per-run username and password, through which
// the app dials tailnet addresses, MagicDNS names and accepted subnet routes.
//
// Protocol: JSON lines. The first stdin line is the config (state key,
// hostname, accept-routes). Later lines are commands. Stdout carries events.
// When stdin closes (the app quit or crashed) the sidecar exits.
//
// Privacy: built with -tags ts_omit_logtail, and log upload is disabled at
// runtime as well, so nothing is sent to log.tailscale.com. The node state
// (keys) is stored encrypted with a key the app keeps in the OS keychain.
package main

import (
	"bufio"
	"context"
	"crypto/rand"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log"
	"net"
	"os"
	"sort"
	"strings"
	"sync"
	"time"

	"tailscale.com/envknob"
	"tailscale.com/ipn"
	"tailscale.com/ipn/ipnstate"
	"tailscale.com/logtail"
	"tailscale.com/net/socks5"
	"tailscale.com/tsnet"
)

type config struct {
	// Base64 32-byte key for the encrypted state file.
	Key          string `json:"key"`
	Dir          string `json:"dir"`
	Hostname     string `json:"hostname"`
	AcceptRoutes bool   `json:"acceptRoutes"`
	// Optional: a self-hosted control server (Headscale). Empty = Tailscale.
	ControlURL string `json:"controlUrl"`
}

type command struct {
	Cmd  string `json:"cmd"`
	On   bool   `json:"on"`
	Port int    `json:"port"`
	To   string `json:"to"`
}

var (
	outMu sync.Mutex
	out   = json.NewEncoder(os.Stdout)
)

func emit(v any) {
	outMu.Lock()
	defer outMu.Unlock()
	_ = out.Encode(v)
}

func fail(format string, a ...any) {
	emit(map[string]any{"ev": "error", "message": fmt.Sprintf(format, a...)})
}

func randHex(n int) string {
	b := make([]byte, n)
	if _, err := rand.Read(b); err != nil {
		panic(err)
	}
	return hex.EncodeToString(b)
}

func main() {
	// No log uploads, whatever the build.
	envknob.SetNoLogsNoSupport()
	logtail.Disable()
	log.SetOutput(io.Discard)
	debug := os.Getenv("NIGHTGRID_TAILNET_DEBUG") != ""

	in := bufio.NewScanner(os.Stdin)
	in.Buffer(make([]byte, 64*1024), 64*1024)
	if !in.Scan() {
		return
	}
	var cfg config
	if err := json.Unmarshal(in.Bytes(), &cfg); err != nil {
		fail("bad config: %v", err)
		os.Exit(2)
	}
	key, err := base64.StdEncoding.DecodeString(cfg.Key)
	if err != nil || len(key) != 32 || cfg.Dir == "" {
		fail("bad config: need a 32-byte key and a state folder")
		os.Exit(2)
	}
	if err := os.MkdirAll(cfg.Dir, 0o700); err != nil {
		fail("state folder: %v", err)
		os.Exit(2)
	}
	store, err := openStore(cfg.Dir, key)
	if err != nil {
		fail("node state: %v", err)
		os.Exit(2)
	}

	logf := func(string, ...any) {}
	if debug {
		logf = func(f string, a ...any) { fmt.Fprintf(os.Stderr, f+"\n", a...) }
	}
	srv := &tsnet.Server{
		Dir:        cfg.Dir,
		Store:      store,
		Hostname:   cfg.Hostname,
		ControlURL: cfg.ControlURL,
		Logf:       logf,
		UserLogf:   logf,
	}
	if err := srv.Start(); err != nil {
		fail("start: %v", err)
		os.Exit(1)
	}
	defer srv.Close()
	lc, err := srv.LocalClient()
	if err != nil {
		fail("local client: %v", err)
		os.Exit(1)
	}

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// SOCKS5 for the app only: loopback, random port, per-run credentials.
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		fail("proxy: %v", err)
		os.Exit(1)
	}
	user, pass := randHex(8), randHex(16)
	go func() {
		s := &socks5.Server{Logf: logf, Dialer: srv.Dial, Username: user, Password: pass}
		_ = s.Serve(ln)
	}()
	emit(map[string]any{"ev": "ready", "socks": ln.Addr().String(), "user": user, "pass": pass})

	setRoutes := func(on bool) {
		_, err := lc.EditPrefs(ctx, &ipn.MaskedPrefs{Prefs: ipn.Prefs{RouteAll: on}, RouteAllSet: true})
		if err != nil {
			fail("accept routes: %v", err)
		}
	}

	// Status goes out at most twice a second, however busy the bus is.
	dirty := make(chan struct{}, 1)
	changed := func() {
		select {
		case dirty <- struct{}{}:
		default:
		}
	}
	go func() {
		for {
			select {
			case <-ctx.Done():
				return
			case <-dirty:
				sendStatus(ctx, lc)
				time.Sleep(500 * time.Millisecond)
			}
		}
	}()

	// Follow the backend: login links, state changes, peer updates.
	go func() {
		for {
			w, err := lc.WatchIPNBus(ctx, ipn.NotifyInitialState|ipn.NotifyPeerChanges)
			if err != nil {
				if ctx.Err() != nil {
					return
				}
				time.Sleep(time.Second)
				continue
			}
			routesSet := false
			for {
				n, err := w.Next()
				if err != nil {
					break
				}
				if n.BrowseToURL != nil && *n.BrowseToURL != "" {
					emit(map[string]any{"ev": "login", "url": *n.BrowseToURL})
				}
				if n.ErrMessage != nil {
					fail("%s", *n.ErrMessage)
				}
				if n.State != nil {
					emit(map[string]any{"ev": "state", "state": n.State.String()})
					if *n.State == ipn.Running && !routesSet {
						routesSet = true
						setRoutes(cfg.AcceptRoutes)
					}
				}
				if n.State != nil || n.SelfChange != nil || n.PeersChanged != nil || n.PeersRemoved != nil || n.PeerChangedPatch != nil || n.LoginFinished != nil {
					changed()
				}
			}
			w.Close()
			if ctx.Err() != nil {
				return
			}
		}
	}()

	// Profile sync (D-203): tailnet connections to one port go to the app's
	// listener on loopback, so paired devices on the built-in tailnet reach it.
	var fwd net.Listener
	defer func() {
		if fwd != nil {
			fwd.Close()
		}
	}()

	for in.Scan() {
		var c command
		if err := json.Unmarshal(in.Bytes(), &c); err != nil {
			fail("bad command: %v", err)
			continue
		}
		switch c.Cmd {
		case "login":
			if err := lc.StartLoginInteractive(ctx); err != nil {
				fail("login: %v", err)
			}
		case "logout":
			if err := lc.Logout(ctx); err != nil {
				fail("log out: %v", err)
			}
			changed()
		case "acceptRoutes":
			cfg.AcceptRoutes = c.On
			setRoutes(c.On)
			changed()
		case "status":
			changed()
		case "forward":
			if fwd != nil {
				fwd.Close()
				fwd = nil
			}
			if c.Port > 0 && c.Port < 65536 && isLoopback(c.To) {
				l, err := srv.Listen("tcp", fmt.Sprintf(":%d", c.Port))
				if err != nil {
					fail("sync listener: %v", err)
					break
				}
				fwd = l
				go forward(l, c.To)
			}
		case "quit":
			return
		default:
			fail("unknown command %q", c.Cmd)
		}
	}
	// stdin closed: the app is gone.
}

type peer struct {
	Name    string   `json:"name"`
	DNSName string   `json:"dnsName"`
	IPs     []string `json:"ips"`
	OS      string   `json:"os"`
	Online  bool     `json:"online"`
	Routes  []string `json:"routes"`
}

type statusClient interface {
	Status(ctx context.Context) (*ipnstate.Status, error)
}

func sendStatus(ctx context.Context, lc statusClient) {
	c, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()
	st, err := lc.Status(c)
	if err != nil {
		if !errors.Is(err, context.Canceled) {
			fail("status: %v", err)
		}
		return
	}
	ev := map[string]any{"ev": "status", "state": st.BackendState, "authUrl": st.AuthURL}
	suffix := st.MagicDNSSuffix
	if st.CurrentTailnet != nil {
		ev["tailnet"] = st.CurrentTailnet.Name
		suffix = st.CurrentTailnet.MagicDNSSuffix
	}
	ev["magicDnsSuffix"] = suffix
	if st.Self != nil {
		ev["self"] = toPeer(st.Self)
	}
	peers := make([]peer, 0, len(st.Peer))
	for _, p := range st.Peer {
		peers = append(peers, toPeer(p))
	}
	sort.Slice(peers, func(i, j int) bool { return peers[i].Name < peers[j].Name })
	ev["peers"] = peers
	emit(ev)
}

func toPeer(p *ipnstate.PeerStatus) peer {
	ips := make([]string, 0, len(p.TailscaleIPs))
	for _, a := range p.TailscaleIPs {
		ips = append(ips, a.String())
	}
	routes := []string{}
	if p.PrimaryRoutes != nil {
		for _, r := range p.PrimaryRoutes.All() {
			routes = append(routes, r.String())
		}
	}
	name := strings.SplitN(strings.TrimSuffix(p.DNSName, "."), ".", 2)[0]
	if name == "" {
		name = p.HostName
	}
	return peer{Name: name, DNSName: strings.TrimSuffix(p.DNSName, "."), IPs: ips, OS: p.OS, Online: p.Online, Routes: routes}
}

// isLoopback accepts only 127.0.0.1:port / [::1]:port targets.
func isLoopback(addr string) bool {
	host, _, err := net.SplitHostPort(addr)
	if err != nil {
		return false
	}
	ip := net.ParseIP(host)
	return ip != nil && ip.IsLoopback()
}

// forward copies each tailnet connection to the app's listener.
func forward(l net.Listener, to string) {
	for {
		c, err := l.Accept()
		if err != nil {
			return
		}
		go func(c net.Conn) {
			defer c.Close()
			d, err := net.DialTimeout("tcp", to, 5*time.Second)
			if err != nil {
				return
			}
			defer d.Close()
			done := make(chan struct{}, 2)
			go func() { _, _ = io.Copy(d, c); done <- struct{}{} }()
			go func() { _, _ = io.Copy(c, d); done <- struct{}{} }()
			<-done
		}(c)
	}
}
