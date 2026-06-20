package setup

import (
	"bytes"
	"crypto/rand"
	"encoding/base64"
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

	"github.com/google/uuid"
	"github.com/tsonubin/sub-maker/internal/assets"
	"github.com/tsonubin/sub-maker/internal/config"
	"github.com/tsonubin/sub-maker/internal/diagnostics"
	"github.com/tsonubin/sub-maker/internal/generator"
	"gopkg.in/yaml.v3"
)

// Apply performs the full "setup on current server".
// Downloads sing-box + subconverter, renders real configs from templates using collected data,
// writes nodes, systemd units, sub-maker config, certificates, and prints firewall hints.
func Apply(cfg *config.SetupConfig) error {
	if cfg == nil {
		cfg = config.DefaultConfig()
	}

	base := config.EtcBase()
	etcSub := filepath.Join(base, "sub-maker")
	etcSB := filepath.Join(base, "sing-box")
	optSC := subconverterDir()

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
	cfgBytes, err := yaml.Marshal(cfg)
	if err != nil {
		return fmt.Errorf("marshal config: %w", err)
	}
	cfgPath := filepath.Join(etcSub, "config.yaml")
	if err := os.WriteFile(cfgPath, cfgBytes, 0640); err != nil {
		return err
	}

	if !config.DemoMode() {
		if err := startAndVerifyServices(cfg); err != nil {
			return err
		}
	}

	printApplySummary(cfg, nodesPath, etcSB, optSC, cfgPath)
	return nil
}

func printApplySummary(cfg *config.SetupConfig, nodesPath, etcSB, optSC, cfgPath string) {
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

	certsDir := filepath.Join(config.EtcBase(), "sub-maker", "certs")
	certPath := filepath.Join(certsDir, "fullchain.pem")
	keyPath := filepath.Join(certsDir, "privkey.pem")
	if cfg.CertPath == "" {
		cfg.CertPath = certPath
	}
	if cfg.KeyPath == "" {
		cfg.KeyPath = keyPath
	}
	if err := diagnostics.ValidateCertificatePair(cfg.CertPath, cfg.KeyPath, cfg.Domain); err == nil {
		return nil
	}

	switch cfg.CertMode {
	case config.CertStrategyCertbotHTTP:
		if !config.DemoMode() {
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
		if !config.DemoMode() {
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
	for _, proto := range cfg.EnabledProtocols {
		creds := cfg.Creds[proto]
		if creds == nil {
			creds = map[string]string{}
			cfg.Creds[proto] = creds
		}
		switch proto {
		case "reality":
			if creds["uuid"] == "" {
				creds["uuid"] = uuid.NewString()
			}
			if creds["short_id"] == "" {
				creds["short_id"] = randomHex(4)
			}
			if creds["private_key"] == "" || creds["pbk"] == "" {
				privateKey, publicKey, err := generateRealityKeypair()
				if err != nil {
					return err
				}
				creds["private_key"] = privateKey
				creds["pbk"] = publicKey
			}
		case "hysteria2", "tuic", "anytls":
			if creds["pass"] == "" {
				creds["pass"] = randomSecret()
			}
			if proto == "tuic" && creds["uuid"] == "" {
				creds["uuid"] = uuid.NewString()
			}
		case "ss2022":
			if creds["pass"] == "" {
				creds["pass"] = randomSS2022Secret()
			}
		}
	}
	return nil
}

func randomSecret() string {
	b := make([]byte, 18)
	if _, err := rand.Read(b); err != nil {
		return uuid.NewString()
	}
	return base64.RawURLEncoding.EncodeToString(b)
}

func randomSS2022Secret() string {
	b := make([]byte, 16)
	if _, err := rand.Read(b); err != nil {
		return randomSecret()
	}
	return base64.StdEncoding.EncodeToString(b)
}

func randomHex(size int) string {
	b := make([]byte, size)
	if _, err := rand.Read(b); err != nil {
		return strings.ReplaceAll(uuid.NewString(), "-", "")[:size*2]
	}
	const digits = "0123456789abcdef"
	out := make([]byte, size*2)
	for i, value := range b {
		out[i*2] = digits[value>>4]
		out[i*2+1] = digits[value&0x0f]
	}
	return string(out)
}

func generateRealityKeypair() (string, string, error) {
	binary := "/usr/local/bin/sing-box"
	if config.DemoMode() {
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

func startAndVerifyServices(cfg *config.SetupConfig) error {
	if err := diagnostics.RunSystemctl("daemon-reload"); err != nil {
		return err
	}
	services := []string{"sing-box", "sub-maker-sub"}
	if SubconverterInstalled() {
		services = append(services, "subconverter")
	}
	enableArgs := append([]string{"enable", "--now"}, services...)
	if err := diagnostics.RunSystemctl(enableArgs...); err != nil {
		return err
	}
	localURL := fmt.Sprintf("http://127.0.0.1:%d/sub?token=%s", cfg.SubPort, cfg.SubToken)
	return diagnostics.VerifySubscriptionEndpoint(localURL)
}

func subconverterDir() string {
	if config.DemoMode() {
		return filepath.Join(homeDir(), ".local/subconverter")
	}
	return "/opt/subconverter"
}

func homeDir() string {
	if home := os.Getenv("HOME"); home != "" {
		return home
	}
	return "/tmp"
}

// SubconverterInstalled reports whether the subconverter binary is present.
func SubconverterInstalled() bool {
	candidates := []string{"/opt/subconverter/subconverter"}
	if config.DemoMode() {
		candidates = append([]string{filepath.Join(homeDir(), ".local/subconverter/subconverter")}, candidates...)
	}
	for _, path := range candidates {
		info, err := os.Stat(path)
		if err == nil && !info.IsDir() {
			return true
		}
	}
	return false
}

func protocolServerName(proto string, cfg *config.SetupConfig, creds map[string]string) string {
	if sni := creds["sni"]; sni != "" {
		return sni
	}
	if proto == "reality" {
		return "www.apple.com"
	}
	return cfg.Domain
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

func inboundTemplateData(proto string, cfg *config.SetupConfig) (string, map[string]interface{}) {
	creds := cfg.Creds[proto]
	if creds == nil {
		creds = map[string]string{}
	}
	data := map[string]interface{}{
		"Port":       cfg.Ports[proto],
		"ServerName": protocolServerName(proto, cfg, creds),
	}
	if cfg.CertPath != "" {
		data["CertPath"] = cfg.CertPath
		data["KeyPath"] = cfg.KeyPath
	} else {
		data["CertPath"] = "/etc/sub-maker/certs/fullchain.pem"
		data["KeyPath"] = "/etc/sub-maker/certs/privkey.pem"
	}

	var tplName string
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
	return tplName, data
}

func renderSingBoxConfig(cfg *config.SetupConfig, etcSB string) error {
	baseTpl, err := readTemplate("templates/singbox_base.json.tpl")
	if err != nil {
		baseTpl = []byte(`{"log":{"level":"info"},"dns":{"strategy":"ipv4_only","servers":[{"type":"https","server":"1.1.1.1"}]},"inbounds":[{{range $i, $in := .Inbounds}}{{if $i}},{{end}}{{$in}}{{end}}],"outbounds":[{"type":"direct"}]}`)
	}

	var inboundStrs []string
	for _, proto := range cfg.EnabledProtocols {
		tplName, data := inboundTemplateData(proto, cfg)
		tplContent, err := readTemplate("templates/" + tplName)
		if err != nil {
			tplContent = []byte(fmt.Sprintf(`{"type":"%s","listen":"::","listen_port":%d}`, proto, cfg.Ports[proto]))
		}
		t, err := template.New(proto).Parse(string(tplContent))
		if err != nil {
			return fmt.Errorf("parse %s template: %w", proto, err)
		}
		var buf bytes.Buffer
		if err := t.Execute(&buf, data); err != nil {
			return fmt.Errorf("execute %s template: %w", proto, err)
		}
		inboundStrs = append(inboundStrs, buf.String())
	}

	t, err := template.New("base").Parse(string(baseTpl))
	if err != nil {
		return fmt.Errorf("parse sing-box base template: %w", err)
	}
	var out bytes.Buffer
	if err := t.Execute(&out, map[string][]string{"Inbounds": inboundStrs}); err != nil {
		return fmt.Errorf("execute sing-box base template: %w", err)
	}

	var js map[string]interface{}
	if err := json.Unmarshal(out.Bytes(), &js); err != nil {
		slog.Warn("sing-box config may not be perfect json", "err", err)
	}
	return os.WriteFile(filepath.Join(etcSB, "config.json"), out.Bytes(), 0644)
}

func setupSubconverterFiles(etcSub, optSC string) error {
	pref, err := readTemplate("templates/subconverter_pref.ini.tpl")
	if err != nil || len(pref) == 0 {
		pref = []byte("api_access_token=submaker123\n")
	}
	if err := os.WriteFile(filepath.Join(optSC, "pref.ini"), pref, 0644); err != nil {
		return err
	}

	// download a common ACL4SSR rules config for subconverter to use via &config=
	aclURL := "https://raw.githubusercontent.com/ACL4SSR/ACL4SSR/master/Clash/config/ACL4SSR_Online_Full.ini"
	aclPath := filepath.Join(etcSub, "acl4ssr.ini")
	if err := downloadFile(aclURL, aclPath); err != nil {
		slog.Warn("could not download ACL rules, using minimal", "err", err)
		if err := os.WriteFile(aclPath, []byte("[custom]\n; add your rules here or use remote\n"), 0644); err != nil {
			return err
		}
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
		t, err := template.New(name).Parse(string(content))
		if err != nil {
			return fmt.Errorf("parse unit template %s: %w", name, err)
		}
		var buf bytes.Buffer
		if err := t.Execute(&buf, map[string]string{
			"Token":     cfg.SubToken,
			"Port":      strconv.Itoa(cfg.SubPort),
			"NodesPath": nodesPath,
		}); err != nil {
			return fmt.Errorf("execute unit template %s: %w", name, err)
		}
		path := "/etc/systemd/system/" + name
		if config.DemoMode() {
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
