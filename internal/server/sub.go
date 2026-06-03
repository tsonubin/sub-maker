package server

import (
	"bufio"
	"bytes"
	"fmt"
	"log/slog"
	"net/http"
	"net/url"
	"os"
	"strings"
	"time"
)

func Run() error {
	port := os.Getenv("SUB_MAKER_PORT")
	if port == "" {
		port = "8964"
	}
	token := os.Getenv("SUB_MAKER_TOKEN")
	if token == "" {
		token = "changeme-in-config"
	}

	nodesPath := os.Getenv("SUB_MAKER_NODES_FILE")
	if nodesPath == "" {
		nodesPath = "/etc/sub-maker/nodes.txt"
		if os.Getenv("SUB_MAKER_DEMO") != "" {
			nodesPath = "/tmp/sub-maker-demo-etc/sub-maker/nodes.txt"
		}
	}

	mux := http.NewServeMux()

	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		fmt.Fprintln(w, "sub-maker on "+port)
		fmt.Fprintln(w, "GET /sub?token=XXX  (Clash subscription with common rulesets)")
		fmt.Fprintln(w, "Nodes source:", nodesPath)
		fmt.Fprintln(w, "Also: /raw  (original node URIs)")
	})

	mux.HandleFunc("/raw", func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Query().Get("token") != token {
			http.Error(w, "unauthorized", http.StatusUnauthorized)
			return
		}
		w.Header().Set("Content-Type", "text/plain")
		data, _ := os.ReadFile(nodesPath)
		if len(data) == 0 {
			data = []byte("# no nodes.txt found\n")
		}
		w.Write(data)
	})

	mux.HandleFunc("/sub", func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Query().Get("token") != token {
			http.Error(w, "unauthorized", http.StatusUnauthorized)
			return
		}

		nodes := loadNodes(nodesPath)
		proxiesYAML, proxyNames := nodesToClashProxiesWithNames(nodes)

		w.Header().Set("Content-Type", "text/yaml; charset=utf-8")
		w.Header().Set("Profile-Update-Interval", "6")
		w.Header().Set("Subscription-Userinfo", "upload=0; download=0; total=0; expire=0")

		groupProxies := strings.Join(proxyNames, "\n      - ")
		if groupProxies == "" {
			groupProxies = "DIRECT"
		}

		fmt.Fprintf(w, `mixed-port: 7890
allow-lan: false
mode: rule
log-level: info
external-controller: 127.0.0.1:9090

dns:
  enable: true
  nameserver:
    - 223.5.5.5

proxies:
%s

proxy-groups:
  - name: "🔰 节点选择"
    type: select
    proxies:
      - %s
      - DIRECT
  - name: "🎯 全球直连"
    type: select
    proxies:
      - DIRECT
      - "🔰 节点选择"

rules:
  # Common rulesets (via subconverter + ACL4SSR in full setups)
  - DOMAIN-SUFFIX,local,🎯 全球直连
  - GEOIP,CN,🎯 全球直连
  - RULE-SET,https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/LocalAreaNetwork.list,🎯 全球直连
  - RULE-SET,https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/Google.list,🔰 节点选择
  - RULE-SET,https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/Netflix.list,🔰 节点选择
  - RULE-SET,https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/YouTube.list,🔰 节点选择
  - RULE-SET,https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/Telegram.list,🔰 节点选择
  - RULE-SET,https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/OpenAI.list,🔰 节点选择
  - RULE-SET,https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/BanAD.list,REJECT
  - RULE-SET,https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/ChinaDomain.list,🎯 全球直连
  - MATCH,🔰 节点选择
`, proxiesYAML, strings.ReplaceAll(groupProxies, "\n      - ", "\n      - "))
	})

	srv := &http.Server{
		Addr:         "0.0.0.0:" + port,
		Handler:      mux,
		ReadTimeout:  10 * time.Second,
		WriteTimeout: 30 * time.Second,
	}
	slog.Info("starting sub-maker subscription server", "addr", srv.Addr, "nodes", nodesPath)
	return srv.ListenAndServe()
}

func loadNodes(path string) []string {
	f, err := os.Open(path)
	if err != nil {
		// fallback demo nodes
		return []string{
			"hysteria2://demo-pass@127.0.0.1:8443/?sni=example.com#Hy2",
			"vless://demo-uuid@127.0.0.1:443?security=reality&fp=chrome&sni=www.apple.com&sid=0123abc&pbk=demo-pbk#Reality",
		}
	}
	defer f.Close()

	var out []string
	sc := bufio.NewScanner(f)
	for sc.Scan() {
		line := strings.TrimSpace(sc.Text())
		if line != "" && !strings.HasPrefix(line, "#") {
			out = append(out, line)
		}
	}
	if len(out) == 0 {
		out = []string{
			"hysteria2://demo-pass@127.0.0.1:8443/?sni=example.com#Hy2",
			"vless://demo-uuid@127.0.0.1:443?security=reality&fp=chrome&sni=www.apple.com&sid=0123abc&pbk=demo-pbk#Reality",
		}
	}
	return out
}

func nodesToClashProxiesWithNames(nodes []string) (string, []string) {
	var buf bytes.Buffer
	var names []string
	for _, n := range nodes {
		u, err := url.Parse(n)
		if err != nil {
			continue
		}
		scheme := u.Scheme
		host := u.Hostname()
		port := u.Port()
		if port == "" {
			port = "443"
		}
		remark := u.Fragment
		if remark == "" {
			remark = scheme
		}

		var line string
		switch scheme {
		case "hysteria2":
			line = fmt.Sprintf("  - {name: \"%s\", type: hysteria2, server: %s, port: %s, password: \"%s\", sni: \"%s\"}\n",
				remark, host, port, u.User.Username(), u.Query().Get("sni"))
		case "vless":
			uuid := u.User.Username()
			sec := u.Query().Get("security")
			if sec == "reality" {
				line = fmt.Sprintf("  - {name: \"%s\", type: vless, server: %s, port: %s, uuid: \"%s\", network: tcp, tls: true, reality-opts: {public-key: \"%s\", short-id: \"%s\"}, client-fingerprint: chrome}\n",
					remark, host, port, uuid, u.Query().Get("pbk"), u.Query().Get("sid"))
			} else {
				line = fmt.Sprintf("  - {name: \"%s\", type: vless, server: %s, port: %s, uuid: \"%s\", tls: true}\n", remark, host, port, uuid)
			}
		case "tuic":
			parts := strings.Split(u.User.String(), ":")
			uuid, pass := parts[0], ""
			if len(parts) > 1 {
				pass = parts[1]
			}
			line = fmt.Sprintf("  - {name: \"%s\", type: tuic, server: %s, port: %s, uuid: \"%s\", password: \"%s\", sni: \"%s\", alpn: [h3], udp-relay-mode: native}\n",
				remark, host, port, uuid, pass, u.Query().Get("sni"))
		case "anytls":
			line = fmt.Sprintf("  - {name: \"%s\", type: trojan, server: %s, port: %s, password: \"%s\", sni: \"%s\"}  # anytls (mapped)\n",
				remark, host, port, u.User.Username(), u.Query().Get("sni"))
		case "ss":
			line = fmt.Sprintf("  - {name: \"%s\", type: ss, server: %s, port: %s, cipher: \"%s\", password: \"%s\"}\n",
				remark, host, port, "2022-blake3-aes-128-gcm", u.User.Username())
		default:
			line = fmt.Sprintf("  - {name: \"%s\", type: ss, server: %s, port: %s, cipher: \"aes-128-gcm\", password: \"demo\"}\n", remark, host, port)
		}
		if line != "" {
			buf.WriteString(line)
			names = append(names, remark)
		}
	}
	return buf.String(), names
}

// keep old name for compatibility if any other code calls it
func nodesToClashProxies(nodes []string) string {
	y, _ := nodesToClashProxiesWithNames(nodes)
	return y
}

