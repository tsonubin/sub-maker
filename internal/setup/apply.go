package setup

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io/fs"
	"log/slog"
	"os"
	"path/filepath"
	"strings"
	"text/template"
	"time"

	"github.com/tsonubin/sub-maker/internal/assets"
	"github.com/tsonubin/sub-maker/internal/config"
	"github.com/tsonubin/sub-maker/internal/generator"
	"gopkg.in/yaml.v3"
)

// Apply performs the full "setup on current server".
// Downloads sing-box + subconverter, renders real configs from templates using collected data,
// writes nodes, systemd units, sub-maker config, and (stubs for now) cert/acme + firewall.
func Apply(cfg *config.SetupConfig) error {
	if cfg == nil {
		cfg = config.DefaultConfig()
	}

	// Real paths (or demo paths)
	base := "/etc"
	if os.Getenv("SUB_MAKER_DEMO") != "" {
		base = "/tmp/sub-maker-demo-etc"
	}
	etcSub := filepath.Join(base, "sub-maker")
	etcSB := filepath.Join(base, "sing-box")

	// subconverter install location (system or user-local in demo)
	var optSC string
	if os.Getenv("SUB_MAKER_DEMO") != "" {
		home := os.Getenv("HOME")
		if home == "" {
			home = "/tmp"
		}
		optSC = filepath.Join(home, ".local/subconverter")
	} else {
		optSC = "/opt/subconverter"
	}

	for _, d := range []string{etcSub, etcSB, optSC} {
		if err := os.MkdirAll(d, 0755); err != nil {
			return err
		}
	}

	// 1. Generate and write nodes.txt (used by subconverter file:// )
	nodes := generator.GenerateAll(cfg.ServerAddr, cfg.Domain, cfg.Ports, cfg.Creds)
	nodesPath := filepath.Join(etcSub, "nodes.txt")
	if err := generator.WriteNodesFile(nodesPath, nodes); err != nil {
		return fmt.Errorf("write nodes: %w", err)
	}
	slog.Info("wrote nodes.txt", "count", len(nodes))

	// 2. Download binaries (real)
	slog.Info("downloading sing-box...")
	if err := DownloadSingBox(""); err != nil {
		slog.Warn("sing-box download failed (may already exist or network)", "err", err)
		// continue for demo
	}
	slog.Info("downloading subconverter...")
	if err := DownloadSubconverter(""); err != nil {
		slog.Warn("subconverter download failed", "err", err)
	}

	// 3. Render and write sing-box config using templates + data
	if err := renderSingBoxConfig(cfg, etcSB); err != nil {
		return fmt.Errorf("render sing-box config: %w", err)
	}

	// 4. Write subconverter files (pref + rules)
	if err := setupSubconverterFiles(etcSub, optSC); err != nil {
		slog.Warn("subconverter files", "err", err)
	}

	// 5. Write sub-maker config.yaml
	cfg.UpdatedAt = time.Now()
	if cfg.CreatedAt.IsZero() {
		cfg.CreatedAt = cfg.UpdatedAt
	}
	cfgBytes, _ := yaml.Marshal(cfg)
	cfgPath := filepath.Join(etcSub, "config.yaml")
	if err := os.WriteFile(cfgPath, cfgBytes, 0640); err != nil {
		return err
	}

	// 6. Write systemd units (using templates)
	if err := writeSystemdUnits(cfg, etcSub); err != nil {
		slog.Warn("systemd units", "err", err)
	}

	// 7. Certs (via acme.sh)
	if os.Getenv("SUB_MAKER_DEMO") == "" && cfg.Domain != "" && cfg.CertMode != "self-signed" {
		slog.Info("obtaining cert via acme.sh for " + cfg.Domain)
		if err := EnsureAcme(cfg.Domain, cfg.Email, cfg.CertMode, cfg.ACMETokenCF); err != nil {
			slog.Warn("acme failed, certs may not be ready - run manually or use self-signed", "err", err)
		} else {
			if err := CopyCertsToEtc(cfg.Domain); err != nil {
				slog.Warn("copy certs failed", "err", err)
			} else {
				cfg.CertPath = "/etc/sub-maker/certs/fullchain.pem"
				cfg.KeyPath = "/etc/sub-maker/certs/privkey.pem"
				slog.Info("certs ready at " + cfg.CertPath)
			}
		}
	} else if cfg.Domain != "" && cfg.CertMode == "self-signed" {
		slog.Info("self-signed cert mode selected - generate manually if needed for TLS protocols")
	}

	// 8. Firewall hint
	slog.Info("firewall: ensure ports open", "sub", cfg.SubPort, "others", cfg.Ports)

	fmt.Printf("\n=== APPLY COMPLETE ===\n")
	fmt.Printf("Nodes: %s\n", nodesPath)
	fmt.Printf("sing-box config: %s/config.json\n", etcSB)
	fmt.Printf("subconverter: %s (run manually or via its unit for now)\n", optSC)
	fmt.Printf("sub-maker config: %s\n", cfgPath)
	fmt.Printf("\nYour Clash subscription (after starting services):\n")
	fmt.Printf("  http://%s:%d/sub?token=%s\n", cfg.ServerAddr, cfg.SubPort, cfg.SubToken)
	fmt.Printf("\nNext manual steps (or enhance Apply):\n")
	fmt.Printf("  systemctl daemon-reload\n")
	fmt.Printf("  systemctl enable --now sing-box subconverter sub-maker-sub\n")
	fmt.Printf("  (install acme.sh if needed and issue certs for TLS inbounds)\n")
	fmt.Printf("  ufw allow %d/tcp %d/tcp etc.\n", cfg.SubPort, cfg.Ports["reality"])

	return nil
}

func readTemplate(name string) ([]byte, error) {
	// prefer local disk (dev)
	if data, err := os.ReadFile(name); err == nil {
		return data, nil
	}
	base := filepath.Base(name)
	if data, err := os.ReadFile("templates/" + base); err == nil {
		return data, nil
	}
	// embedded (for installed binary) - assets has templates/ subdir
	fsys, err := fs.Sub(assets.Templates, "templates")
	if err == nil {
		if data, err := fs.ReadFile(fsys, base); err == nil {
			return data, nil
		}
	}
	// last resort minimal
	if strings.Contains(name, "singbox_base") {
		return []byte(`{"log":{"level":"info"},"dns":{"servers":[{"address":"https://1.1.1.1/dns-query"}]},"inbounds":[{{.Inbounds}}],"outbounds":[{"type":"direct"}]}`), nil
	}
	return nil, fmt.Errorf("template %s not found", name)
}

func renderSingBoxConfig(cfg *config.SetupConfig, etcSB string) error {
	// Load base template
	baseTpl, err := readTemplate("templates/singbox_base.json.tpl")
	if err != nil {
		baseTpl = []byte(`{"log":{"level":"info"},"dns":{"servers":[{"address":"https://1.1.1.1/dns-query"}]},"inbounds":[{{.Inbounds}}],"outbounds":[{"type":"direct"}]}`)
	}

	// Build inbound JSON strings from per-protocol templates + data
	var inboundStrs []string
	for _, proto := range cfg.EnabledProtocols {
		port := cfg.Ports[proto]
		creds := cfg.Creds[proto]
		if creds == nil {
			creds = map[string]string{}
		}
		var tplName string
		data := map[string]interface{}{
			"Port":       port,
			"ServerName": cfg.Domain,
		}
		// defaults for certs (overwritten by real in cert step or self-signed)
		if cfg.CertPath != "" {
			data["CertPath"] = cfg.CertPath
			data["KeyPath"] = cfg.KeyPath
		} else {
			data["CertPath"] = "/etc/sub-maker/certs/fullchain.pem"
			data["KeyPath"] = "/etc/sub-maker/certs/privkey.pem"
		}
		switch proto {
		case "reality":
			tplName = "inbound_reality.json.tpl"
			data["UUID"] = creds["uuid"]
			data["PrivateKey"] = "REPLACE_WITH_sing-box_generate_reality-keypair" // user or setup should run sing-box generate
			data["ShortID"] = creds["short_id"]
		case "hysteria2":
			tplName = "inbound_hysteria2.json.tpl"
			data["Password"] = creds["pass"]
		case "tuic":
			tplName = "inbound_tuic.json.tpl"
			data["UUID"] = creds["uuid"]
			data["Password"] = creds["pass"]
		case "anytls":
			tplName = "inbound_anytls.json.tpl"
			data["Password"] = creds["pass"]
		case "ss2022":
			tplName = "inbound_ss2022.json.tpl"
			data["Password"] = creds["pass"]
			data["Method"] = "2022-blake3-aes-128-gcm"
		}
		tplContent, err := readTemplate("templates/" + tplName)
		if err != nil {
			// fallback simple inbound
			tplContent = []byte(fmt.Sprintf(`{"type":"%s","listen":"::","listen_port":%d}`, proto, port))
		}
		t, _ := template.New(proto).Parse(string(tplContent))
		var buf bytes.Buffer
		t.Execute(&buf, data)
		// pretty? no need, sing-box accepts
		inboundStrs = append(inboundStrs, buf.String())
	}

	inboundsJSON := strings.Join(inboundStrs, ",")

	t, _ := template.New("base").Parse(string(baseTpl))
	var out bytes.Buffer
	t.Execute(&out, map[string]string{"Inbounds": inboundsJSON})

	// validate it's json-ish
	var js map[string]interface{}
	if err := json.Unmarshal(out.Bytes(), &js); err != nil {
		slog.Warn("sing-box config may not be perfect json", "err", err)
	}

	return os.WriteFile(filepath.Join(etcSB, "config.json"), out.Bytes(), 0644)
}

func setupSubconverterFiles(etcSub, optSC string) error {
	// copy pref
	pref, _ := os.ReadFile("templates/subconverter_pref.ini.tpl")
	if len(pref) == 0 {
		pref = []byte("api_access_token=submaker123\n")
	}
	os.WriteFile(filepath.Join(optSC, "pref.ini"), pref, 0644)

	// download a common ACL4SSR rules config for subconverter to use via &config=
	aclURL := "https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/config/ACL4SSR_Online_Full.ini"
	aclPath := filepath.Join(etcSub, "acl4ssr.ini")
	if err := downloadFile(aclURL, aclPath); err != nil {
		slog.Warn("could not download ACL rules, using minimal", "err", err)
		os.WriteFile(aclPath, []byte("[custom]\n; add your rules here or use remote\n"), 0644)
	}
	return nil
}

func writeSystemdUnits(cfg *config.SetupConfig, etcSub string) error {
	unitNames := []string{"sing-box.service", "subconverter.service", "sub-maker-sub.service"}
	for _, name := range unitNames {
		content, err := readTemplate("templates/" + name + ".tpl")
		if err != nil {
			content = []byte("[Unit]\nDescription=" + name + "\n[Service]\nExecStart=/bin/true\n")
		}
		t, _ := template.New(name).Parse(string(content))
		var buf bytes.Buffer
		t.Execute(&buf, map[string]string{"Token": cfg.SubToken})
		// for demo, write to /tmp if no perm
		path := "/etc/systemd/system/" + name
		if os.Getenv("SUB_MAKER_DEMO") != "" {
			path = filepath.Join("/tmp", name)
		}
		if err := os.WriteFile(path, buf.Bytes(), 0644); err != nil {
			slog.Warn("write unit", "path", path, "err", err)
		} else {
			slog.Info("wrote unit", "path", path)
		}
	}
	return nil
}

// downloadFile and extract... are in downloader.go

