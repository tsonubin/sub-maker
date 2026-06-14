package setup

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io/fs"
	"log/slog"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"text/template"
	"time"

	"github.com/tsonubin/sub-maker/internal/assets"
	"github.com/tsonubin/sub-maker/internal/config"
	"github.com/tsonubin/sub-maker/internal/diagnostics"
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

	// 1. Download binaries before generating runtime-dependent config such as Reality keypairs.
	slog.Info("downloading sing-box...")
	if err := DownloadSingBox(""); err != nil {
		slog.Warn("sing-box download failed (may already exist or network)", "err", err)
	}
	slog.Info("downloading subconverter...")
	if err := DownloadSubconverter(""); err != nil {
		slog.Warn("subconverter download failed", "err", err)
	}

	// 2. Certs before render, so sing-box config points at real files.
	if err := ensureCertificates(cfg); err != nil {
		return err
	}

	// 3. Runtime credentials before rendering and node generation.
	if err := ensureRuntimeCredentials(cfg); err != nil {
		return err
	}

	// 4. Render and write sing-box config using templates + data.
	if err := renderSingBoxConfig(cfg, etcSB); err != nil {
		return fmt.Errorf("render sing-box config: %w", err)
	}

	// 5. Generate and write selected nodes.txt.
	nodes := generator.GenerateSelected(cfg.ServerAddr, cfg.Domain, cfg.EnabledProtocols, cfg.Ports, cfg.Creds)
	nodesPath := filepath.Join(etcSub, "nodes.txt")
	if err := generator.WriteNodesFile(nodesPath, nodes); err != nil {
		return fmt.Errorf("write nodes: %w", err)
	}
	slog.Info("wrote nodes.txt", "count", len(nodes))

	// 6. Write subconverter files (pref + rules).
	if err := setupSubconverterFiles(etcSub, optSC); err != nil {
		slog.Warn("subconverter files", "err", err)
	}

	// 7. Write systemd units.
	if err := writeSystemdUnits(cfg, nodesPath); err != nil {
		return fmt.Errorf("write systemd units: %w", err)
	}

	// 8. Write sub-maker config.yaml.
	if cfg.LinkPasscodeHash == "" || cfg.LinkPasscodeSalt == "" {
		if err := cfg.SetLinkPasscode(config.GenerateLinkPasscode()); err != nil {
			return fmt.Errorf("set SSH link passcode: %w", err)
		}
	}
	cfg.UpdatedAt = time.Now()
	if cfg.CreatedAt.IsZero() {
		cfg.CreatedAt = cfg.UpdatedAt
	}
	cfgBytes, _ := yaml.Marshal(cfg)
	cfgPath := filepath.Join(etcSub, "config.yaml")
	if err := os.WriteFile(cfgPath, cfgBytes, 0640); err != nil {
		return err
	}

	if os.Getenv("SUB_MAKER_DEMO") == "" {
		if err := startAndVerifyServices(cfg); err != nil {
			return err
		}
	}

	// 9. Firewall hint
	slog.Info("firewall: ensure ports open", "sub", cfg.SubPort, "others", cfg.Ports)

	links := diagnostics.BuildLinks(cfg)
	fmt.Printf("\n=== APPLY COMPLETE ===\n")
	fmt.Printf("Nodes: %s\n", nodesPath)
	fmt.Printf("sing-box config: %s/config.json\n", etcSB)
	fmt.Printf("subconverter: %s\n", optSC)
	fmt.Printf("sub-maker config: %s\n", cfgPath)
	fmt.Printf("\nSubscription:\n  %s\n", links.Subscription)
	fmt.Printf("Raw nodes:\n  %s\n", links.Raw)
	if cfg.Domain != "" && cfg.ServerAddr != "" {
		fmt.Printf("IP fallback:\n  %s\n", links.IPFallback)
	}
	if cfg.LinkPasscodePlain != "" {
		fmt.Printf("SSH link passcode:\n  %s\n", cfg.LinkPasscodePlain)
	}
	fmt.Printf("\nCommands:\n")
	fmt.Printf("  sudo sub-maker status\n")
	fmt.Printf("  sudo sub-maker link\n")
	fmt.Printf("  sudo sub-maker links\n")
	fmt.Printf("  sudo sub-maker doctor\n")
	fmt.Printf("  sudo sub-maker restart\n")
	fmt.Printf("Open firewall ports:\n")
	for _, rule := range firewallRules(cfg) {
		fmt.Printf("  ufw allow %d/%s  # %s\n", rule.Port, rule.Transport, rule.Label)
	}

	return nil
}

type firewallRule struct {
	Port      int
	Transport string
	Label     string
}

func firewallRules(cfg *config.SetupConfig) []firewallRule {
	rules := []firewallRule{{Port: cfg.SubPort, Transport: "tcp", Label: "subscription"}}
	for _, protocol := range cfg.EnabledProtocols {
		port := cfg.Ports[protocol]
		if port <= 0 {
			continue
		}
		switch protocol {
		case "hysteria2", "tuic":
			rules = append(rules, firewallRule{Port: port, Transport: "udp", Label: protocol})
		case "ss2022":
			rules = append(rules,
				firewallRule{Port: port, Transport: "tcp", Label: protocol},
				firewallRule{Port: port, Transport: "udp", Label: protocol},
			)
		default:
			rules = append(rules, firewallRule{Port: port, Transport: "tcp", Label: protocol})
		}
	}
	return rules
}

func ensureCertificates(cfg *config.SetupConfig) error {
	if !diagnostics.ProtocolsRequireLocalCert(cfg.EnabledProtocols) {
		return nil
	}
	if cfg.CertPath != "" && cfg.KeyPath != "" && cfg.CertMode == config.CertStrategyExisting {
		return diagnostics.ValidateCertificatePair(cfg.CertPath, cfg.KeyPath, cfg.Domain)
	}

	certPath := "/etc/sub-maker/certs/fullchain.pem"
	keyPath := "/etc/sub-maker/certs/privkey.pem"
	if os.Getenv("SUB_MAKER_DEMO") != "" {
		certPath = "/tmp/sub-maker-demo-etc/sub-maker/certs/fullchain.pem"
		keyPath = "/tmp/sub-maker-demo-etc/sub-maker/certs/privkey.pem"
	}

	switch cfg.CertMode {
	case config.CertStrategyCertbotHTTP:
		if os.Getenv("SUB_MAKER_DEMO") == "" {
			if err := EnsureCertbot(cfg.Domain, cfg.Email); err != nil {
				return err
			}
			if err := CopyCertbotCertsToEtc(cfg.Domain); err != nil {
				return err
			}
		} else if err := GenerateSelfSignedCert(cfg.Domain, cfg.ServerAddr); err != nil {
			return err
		}
	case config.CertStrategyACMEHTTP, config.CertStrategyACMEDNSCF:
		if os.Getenv("SUB_MAKER_DEMO") == "" {
			if err := EnsureAcme(cfg.Domain, cfg.Email, cfg.CertMode, cfg.ACMETokenCF); err != nil {
				return err
			}
			if err := CopyCertsToEtc(cfg.Domain); err != nil {
				return err
			}
		} else if err := GenerateSelfSignedCert(cfg.Domain, cfg.ServerAddr); err != nil {
			return err
		}
	case config.CertStrategySelfSigned:
		if err := GenerateSelfSignedCert(cfg.Domain, cfg.ServerAddr); err != nil {
			return err
		}
	case config.CertStrategyExisting:
		return diagnostics.ValidateCertificatePair(cfg.CertPath, cfg.KeyPath, cfg.Domain)
	default:
		return fmt.Errorf("unsupported certificate strategy: %s", cfg.CertMode)
	}

	cfg.CertPath = certPath
	cfg.KeyPath = keyPath
	return diagnostics.ValidateCertificatePair(cfg.CertPath, cfg.KeyPath, cfg.Domain)
}

func ensureRuntimeCredentials(cfg *config.SetupConfig) error {
	if cfg.Creds == nil {
		cfg.Creds = make(map[string]map[string]string)
	}
	if !protocolEnabled(cfg.EnabledProtocols, "reality") {
		return nil
	}
	creds := cfg.Creds["reality"]
	if creds == nil {
		creds = map[string]string{}
		cfg.Creds["reality"] = creds
	}
	if creds["private_key"] != "" && creds["pbk"] != "" {
		return nil
	}
	privateKey, publicKey, err := generateRealityKeypair()
	if err != nil {
		return err
	}
	creds["private_key"] = privateKey
	creds["pbk"] = publicKey
	return nil
}

func generateRealityKeypair() (string, string, error) {
	binary := "/usr/local/bin/sing-box"
	if os.Getenv("SUB_MAKER_DEMO") != "" {
		if home := os.Getenv("HOME"); home != "" {
			binary = filepath.Join(home, ".local/bin/sing-box")
		}
	}
	if _, err := os.Stat(binary); err != nil {
		if found, lookErr := exec.LookPath("sing-box"); lookErr == nil {
			binary = found
		}
	}
	cmd := exec.Command(binary, "generate", "reality-keypair")
	out, err := cmd.CombinedOutput()
	if err != nil {
		return "", "", fmt.Errorf("generate reality keypair: %w\n%s", err, string(out))
	}
	return parseRealityKeypairOutput(string(out))
}

func parseRealityKeypairOutput(output string) (string, string, error) {
	privateKey := extractKeyLine(output, `(?i)private[\s_-]*key\s*:\s*([A-Za-z0-9_-]+)`)
	publicKey := extractKeyLine(output, `(?i)public[\s_-]*key\s*:\s*([A-Za-z0-9_-]+)`)
	if privateKey == "" || publicKey == "" {
		return "", "", fmt.Errorf("could not parse sing-box reality keypair output: %s", output)
	}
	return privateKey, publicKey, nil
}

func extractKeyLine(output, pattern string) string {
	re := regexp.MustCompile(pattern)
	matches := re.FindStringSubmatch(output)
	if len(matches) < 2 {
		return ""
	}
	return matches[1]
}

func protocolEnabled(protocols []string, protocol string) bool {
	for _, value := range protocols {
		if value == protocol {
			return true
		}
	}
	return false
}

func startAndVerifyServices(cfg *config.SetupConfig) error {
	if err := diagnostics.RunSystemctl("daemon-reload"); err != nil {
		return err
	}
	if err := diagnostics.RunSystemctl("enable", "--now", "sing-box", "sub-maker-sub"); err != nil {
		return err
	}
	localURL := fmt.Sprintf("http://127.0.0.1:%d/sub?token=%s", cfg.SubPort, cfg.SubToken)
	return diagnostics.VerifySubscriptionEndpoint(localURL)
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
			data["PrivateKey"] = creds["private_key"]
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

func writeSystemdUnits(cfg *config.SetupConfig, nodesPath string) error {
	unitNames := []string{"sing-box.service", "subconverter.service", "sub-maker-sub.service"}
	for _, name := range unitNames {
		content, err := readTemplate("templates/" + name + ".tpl")
		if err != nil {
			if name == "sub-maker-sub.service" {
				return fmt.Errorf("required unit template missing: %s", name)
			}
			content = []byte("[Unit]\nDescription=" + name + "\n[Service]\nExecStart=/bin/true\n")
		}
		t, _ := template.New(name).Parse(string(content))
		var buf bytes.Buffer
		t.Execute(&buf, map[string]string{
			"Token":     cfg.SubToken,
			"Port":      strconv.Itoa(cfg.SubPort),
			"NodesPath": nodesPath,
		})
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
