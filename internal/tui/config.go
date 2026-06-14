package tui

import (
	"context"
	"fmt"
	"log/slog"
	"strings"

	"github.com/charmbracelet/huh"
	"github.com/google/uuid"
	"github.com/tsonubin/sub-maker/internal/config"
	"github.com/tsonubin/sub-maker/internal/diagnostics"
)

func RunConfigWizard(existing *config.SetupConfig) (*config.SetupConfig, error) {
	cfg := cloneConfig(existing)
	applyConfigDefaults(cfg)

	detectedPublicIP := ""
	serverAddrDescription := "Used in generated client links and the subscription URL."
	if detectedIP, err := diagnostics.DetectPublicIP(context.Background()); err == nil {
		detectedPublicIP = detectedIP
		serverAddrDescription = "Detected: " + detectedIP + ". Current value is prefilled."
		slog.Info("detected server public IP", "ip", detectedIP)
	} else {
		slog.Info("could not auto-detect server public IP", "err", err)
	}

	setupMode := string(cfg.SetupMode)
	serverAddr := cfg.ServerAddr
	domain := cfg.Domain
	email := cfg.Email
	token := cfg.SubToken
	subPortStr := fmt.Sprintf("%d", cfg.SubPort)
	linkPasscode := ""

	form := huh.NewForm(huh.NewGroup(
		huh.NewNote().
			Title("sub-maker config").
			Description("Edit the existing setup. Blank secret fields keep their current value."),
		huh.NewSelect[string]().
			Title("Setup mode").
			Options(
				huh.NewOption("Production domain setup", string(config.SetupModeProduction)),
				huh.NewOption("IP-only / advanced setup", string(config.SetupModeIPOnly)),
			).
			Value(&setupMode),
		huh.NewInput().Title("Server public address (IP or domain for client links)").Description(serverAddrDescription).Value(&serverAddr),
		huh.NewInput().
			Title("Domain for DNS, certificates, and domain-based subscription").
			Value(&domain).
			Validate(func(value string) error {
				if config.SetupMode(setupMode) == config.SetupModeProduction && strings.TrimSpace(value) == "" {
					return fmt.Errorf("domain is required for production setup")
				}
				return nil
			}),
		huh.NewInput().Title("ACME contact email").Value(&email),
		huh.NewInput().Title("Subscription token").Description("Changing this invalidates old subscription URLs.").Value(&token),
		huh.NewInput().
			Title("New SSH link retrieval passcode").
			Description("Blank keeps the current passcode. Type a new value to rotate it.").
			Value(&linkPasscode).
			EchoMode(huh.EchoModePassword),
		huh.NewInput().Title("Subscription listen port").Value(&subPortStr),
	))
	if err := form.Run(); err != nil {
		return nil, err
	}

	cfg.SetupMode = config.SetupMode(setupMode)
	cfg.ServerAddr = strings.TrimSpace(serverAddr)
	cfg.Domain = strings.TrimSpace(domain)
	cfg.Email = strings.TrimSpace(email)
	cfg.SubToken = strings.TrimSpace(token)
	if cfg.ServerAddr == "" {
		cfg.ServerAddr = "127.0.0.1"
	}
	if cfg.SubToken == "" {
		cfg.SubToken = randomToken()
	}
	if linkPasscode != "" {
		if err := cfg.SetLinkPasscode(linkPasscode); err != nil {
			return nil, err
		}
	}
	if subPortStr != "" {
		var subPort int
		fmt.Sscanf(subPortStr, "%d", &subPort)
		if subPort > 0 {
			cfg.SubPort = subPort
		}
	}
	if cfg.SubPort == 0 {
		cfg.SubPort = 8964
	}

	if cfg.SetupMode == config.SetupModeProduction && cfg.Domain != "" {
		expectedDNSAddress := cfg.ServerAddr
		if ipv4, ipv6 := splitExpectedIPs(expectedDNSAddress); ipv4 == "" && ipv6 == "" && detectedPublicIP != "" {
			expectedDNSAddress = detectedPublicIP
		}
		if err := verifyDomainDNS(cfg, expectedDNSAddress); err != nil {
			return nil, err
		}
	}

	selected, err := collectProtocolSelection(cfg.EnabledProtocols)
	if err != nil {
		return nil, err
	}
	cfg.EnabledProtocols = selected

	if diagnostics.ProtocolsRequireLocalCert(selected) {
		if err := collectCertStrategy(cfg); err != nil {
			return nil, err
		}
	} else {
		cfg.CertMode = config.CertStrategySelfSigned
	}

	if err := collectProtocolDetails(cfg, selected, true); err != nil {
		return nil, err
	}

	slog.Info("TUI configuration complete", "enabled", cfg.EnabledProtocols, "port", cfg.SubPort)
	return cfg, nil
}

func collectProtocolSelection(current []string) ([]string, error) {
	protocols := []string{"reality", "hysteria2", "tuic", "anytls", "ss2022"}
	protocolLabels := map[string]string{
		"reality":   "VLESS + Reality-Vision",
		"hysteria2": "Hysteria2",
		"tuic":      "TUIC v5",
		"anytls":    "AnyTLS",
		"ss2022":    "Shadowsocks 2022",
	}
	var selected []string
	form := huh.NewForm(huh.NewGroup(
		huh.NewMultiSelect[string]().
			Title("Select protocols to enable").
			Options(
				huh.NewOption(protocolLabels["reality"], "reality").Selected(protocolSelected(current, "reality")),
				huh.NewOption(protocolLabels["hysteria2"], "hysteria2").Selected(protocolSelected(current, "hysteria2")),
				huh.NewOption(protocolLabels["tuic"], "tuic").Selected(protocolSelected(current, "tuic")),
				huh.NewOption(protocolLabels["anytls"], "anytls").Selected(protocolSelected(current, "anytls")),
				huh.NewOption(protocolLabels["ss2022"], "ss2022").Selected(protocolSelected(current, "ss2022")),
			).
			Value(&selected),
	))
	if err := form.Run(); err != nil {
		return nil, err
	}
	if len(selected) == 0 {
		selected = protocols
	}
	return selected, nil
}

func collectProtocolDetails(cfg *config.SetupConfig, selected []string, preserveSecrets bool) error {
	for _, p := range selected {
		port := cfg.Ports[p]
		remark := strings.Title(p)
		extra := cloneStringMap(cfg.Creds[p])
		if extra["remark"] != "" {
			remark = extra["remark"]
		}

		portStr := fmt.Sprintf("%d", port)
		pRemark := remark
		pSNI := extra["sni"]
		if pSNI == "" {
			if p == "reality" {
				pSNI = "www.apple.com"
			} else {
				pSNI = cfg.Domain
			}
		}
		pPass := ""
		pUUID := extra["uuid"]

		form := huh.NewForm(huh.NewGroup(
			huh.NewInput().Title(fmt.Sprintf("[%s] Listen port", remark)).Value(&portStr),
			huh.NewInput().Title(fmt.Sprintf("[%s] Remark / tag for client", remark)).Value(&pRemark),
			huh.NewInput().Title(fmt.Sprintf("[%s] SNI / target domain", remark)).Value(&pSNI),
			huh.NewInput().
				Title(fmt.Sprintf("[%s] Password / secret", remark)).
				Description(secretDescription(extra["pass"], preserveSecrets)).
				Value(&pPass).
				EchoMode(huh.EchoModePassword),
			huh.NewInput().Title(fmt.Sprintf("[%s] UUID", remark)).Description("Blank auto-generates when required.").Value(&pUUID),
		))
		if err := form.Run(); err != nil {
			return err
		}

		var pPort int
		fmt.Sscanf(portStr, "%d", &pPort)
		if pPort > 0 {
			cfg.Ports[p] = pPort
		}
		if pRemark != "" {
			extra["remark"] = pRemark
		}
		if pSNI != "" {
			extra["sni"] = pSNI
		}
		if p == "hysteria2" || p == "tuic" || p == "anytls" || p == "ss2022" {
			switch {
			case pPass != "":
				extra["pass"] = pPass
			case extra["pass"] == "":
				if p == "ss2022" {
					extra["pass"] = randomSS2022Secret()
				} else {
					extra["pass"] = randomSecret()
				}
			}
		}
		if p == "reality" || p == "tuic" {
			if pUUID == "" {
				pUUID = uuid.New().String()
			}
			extra["uuid"] = pUUID
		}

		if p == "reality" {
			pbk := extra["pbk"]
			sid := extra["short_id"]
			privateKey := extra["private_key"]
			formR := huh.NewForm(huh.NewGroup(
				huh.NewInput().
					Title("Reality public key").
					Description("Blank keeps existing; setup auto-generates when keypair is missing.").
					Value(&pbk),
				huh.NewInput().Title("Reality shortId (8 hex)").Value(&sid),
			))
			if err := formR.Run(); err != nil {
				return err
			}
			if pbk != "" {
				extra["pbk"] = pbk
			}
			if privateKey != "" {
				extra["private_key"] = privateKey
			}
			if sid == "" {
				sid = randomHex(4)
			}
			extra["short_id"] = sid
		}

		cfg.Creds[p] = extra
	}
	return nil
}

func secretDescription(current string, preserve bool) string {
	if preserve && current != "" {
		return "Blank keeps the current secret."
	}
	return "Blank auto-generates a new secret."
}

func protocolSelected(protocols []string, protocol string) bool {
	if len(protocols) == 0 {
		return true
	}
	for _, value := range protocols {
		if value == protocol {
			return true
		}
	}
	return false
}

func applyConfigDefaults(cfg *config.SetupConfig) {
	defaults := config.DefaultConfig()
	if cfg.SubPort == 0 {
		cfg.SubPort = defaults.SubPort
	}
	if cfg.SetupMode == "" {
		cfg.SetupMode = defaults.SetupMode
	}
	if cfg.CertMode == "" {
		cfg.CertMode = defaults.CertMode
	}
	if len(cfg.EnabledProtocols) == 0 {
		cfg.EnabledProtocols = append([]string(nil), defaults.EnabledProtocols...)
	}
	if cfg.Ports == nil {
		cfg.Ports = map[string]int{}
	}
	for protocol, port := range defaults.Ports {
		if cfg.Ports[protocol] == 0 {
			cfg.Ports[protocol] = port
		}
	}
	if cfg.Creds == nil {
		cfg.Creds = map[string]map[string]string{}
	}
}

func cloneConfig(in *config.SetupConfig) *config.SetupConfig {
	if in == nil {
		return config.DefaultConfig()
	}
	out := *in
	out.EnabledProtocols = append([]string(nil), in.EnabledProtocols...)
	out.Ports = make(map[string]int, len(in.Ports))
	for key, value := range in.Ports {
		out.Ports[key] = value
	}
	out.Creds = make(map[string]map[string]string, len(in.Creds))
	for key, value := range in.Creds {
		out.Creds[key] = cloneStringMap(value)
	}
	return &out
}

func cloneStringMap(in map[string]string) map[string]string {
	out := make(map[string]string, len(in))
	for key, value := range in {
		out[key] = value
	}
	return out
}
