package tui

import (
	"context"
	"crypto/rand"
	"encoding/base64"
	"fmt"
	"log/slog"
	"net/netip"
	"strings"

	"github.com/charmbracelet/huh"
	"github.com/google/uuid"
	"github.com/tsonubin/sub-maker/internal/config"
	"github.com/tsonubin/sub-maker/internal/diagnostics"
)

// RunWizard runs the full interactive collection using huh (simple yet powerful forms).
// Returns the populated config (ready for Apply).
// This is the main entry from main --setup.
func RunWizard() (*config.SetupConfig, error) {
	cfg := config.DefaultConfig()

	var setupMode string = string(config.SetupModeProduction)
	detectedPublicIP := ""
	serverAddrPlaceholder := "1.2.3.4 or your.domain.com"
	serverAddrDescription := "Used in generated client links and the subscription URL."
	if detectedIP, err := diagnostics.DetectPublicIP(context.Background()); err == nil {
		detectedPublicIP = detectedIP
		cfg.ServerAddr = detectedIP
		serverAddrPlaceholder = detectedIP
		serverAddrDescription = "Detected: " + detectedIP + ". You can replace it with a domain."
		slog.Info("detected server public IP", "ip", detectedIP)
	} else {
		slog.Info("could not auto-detect server public IP", "err", err)
	}

	intro := huh.NewForm(
		huh.NewGroup(
			huh.NewNote().
				Title("sub-maker TUI").
				Description("Production setup verifies DNS, certificates, services, and subscription output before it claims success."),
			huh.NewSelect[string]().
				Title("Setup mode").
				Options(
					huh.NewOption("Production domain setup (recommended)", string(config.SetupModeProduction)),
					huh.NewOption("IP-only / advanced setup", string(config.SetupModeIPOnly)),
				).
				Value(&setupMode),
		),
	)
	if err := intro.Run(); err != nil {
		return nil, err
	}
	cfg.SetupMode = config.SetupMode(setupMode)

	var domain, email, token, subPortStr, linkPasscode string
	serverAddr := cfg.ServerAddr
	addressForm := huh.NewForm(
		huh.NewGroup(
			huh.NewInput().Title("Server public address (IP or domain for client links)").Description(serverAddrDescription).Value(&serverAddr).Placeholder(serverAddrPlaceholder),
			huh.NewInput().
				Title("Domain for DNS, certificates, and domain-based subscription").
				Description("Production mode requires an A/AAAA record pointing at this server before cert issuance. IP-only mode can leave this blank.").
				Value(&domain).
				Placeholder("proxy.example.com").
				Validate(func(value string) error {
					if cfg.SetupMode == config.SetupModeProduction && strings.TrimSpace(value) == "" {
						return fmt.Errorf("domain is required for production setup")
					}
					return nil
				}),
			huh.NewInput().Title("ACME contact email").Value(&email).Placeholder("admin@example.com"),
			huh.NewInput().Title("Subscription token (auto-generated if empty)").Value(&token).Placeholder("leave blank to auto-gen"),
			huh.NewInput().
				Title("SSH link retrieval passcode (blank = auto-generate)").
				Description("Used by `sub-maker link` over SSH to reveal only the subscription URL.").
				Value(&linkPasscode).
				EchoMode(huh.EchoModePassword),
			huh.NewInput().Title("Subscription listen port").Value(&subPortStr).Placeholder("8964"),
		),
	)
	if err := addressForm.Run(); err != nil {
		return nil, err
	}

	cfg.ServerAddr = serverAddr
	cfg.Domain = domain
	cfg.Email = email
	if cfg.ServerAddr == "" {
		cfg.ServerAddr = "127.0.0.1"
	}
	if token == "" {
		token = randomToken()
	}
	cfg.SubToken = token
	if linkPasscode == "" {
		linkPasscode = config.GenerateLinkPasscode()
	}
	if err := cfg.SetLinkPasscode(linkPasscode); err != nil {
		return nil, err
	}
	if subPortStr != "" {
		fmt.Sscanf(subPortStr, "%d", &cfg.SubPort)
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

	// 2. Protocol selection (multi)
	var selected []string
	protocols := []string{"reality", "hysteria2", "tuic", "anytls", "ss2022"}
	protocolLabels := map[string]string{
		"reality":   "VLESS + Reality-Vision (stealth king)",
		"hysteria2": "Hysteria2 (speed on lossy links)",
		"tuic":      "TUIC v5 (great QUIC alt)",
		"anytls":    "AnyTLS (modern padding TLS camo)",
		"ss2022":    "Shadowsocks 2022 (simple & fast)",
	}

	form2 := huh.NewForm(
		huh.NewGroup(
			huh.NewMultiSelect[string]().
				Title("Select protocols to enable (top-5 recommended)").
				Options(
					huh.NewOption(protocolLabels["reality"], "reality").Selected(true),
					huh.NewOption(protocolLabels["hysteria2"], "hysteria2").Selected(true),
					huh.NewOption(protocolLabels["tuic"], "tuic").Selected(true),
					huh.NewOption(protocolLabels["anytls"], "anytls").Selected(true),
					huh.NewOption(protocolLabels["ss2022"], "ss2022").Selected(true),
				).
				Value(&selected),
		),
	)
	if err := form2.Run(); err != nil {
		return nil, err
	}
	if len(selected) == 0 {
		selected = protocols
	}
	cfg.EnabledProtocols = selected

	if diagnostics.ProtocolsRequireLocalCert(selected) {
		if err := collectCertStrategy(cfg); err != nil {
			return nil, err
		}
	} else {
		cfg.CertMode = config.CertStrategySelfSigned
	}

	// 3. Per-protocol details (ports, creds, remarks) - sequential for simplicity.
	for _, p := range selected {
		port := cfg.Ports[p]
		remark := strings.Title(p)
		extra := map[string]string{}

		portStr := fmt.Sprintf("%d", port)
		var pRemark, pSNI, pPass, pUUID string
		if p == "reality" {
			pSNI = "www.apple.com"
		} else {
			pSNI = cfg.Domain
		}

		formP := huh.NewForm(
			huh.NewGroup(
				huh.NewInput().Title(fmt.Sprintf("[%s] Listen port", remark)).Value(&portStr).Placeholder(portStr),
				huh.NewInput().Title(fmt.Sprintf("[%s] Remark / tag for client", remark)).Value(&pRemark).Placeholder(remark),
				huh.NewInput().Title(fmt.Sprintf("[%s] SNI / target domain (for TLS or Reality)", remark)).Value(&pSNI).Placeholder(cfg.Domain),
				huh.NewInput().Title(fmt.Sprintf("[%s] Password / secret (blank = auto)", remark)).Value(&pPass).EchoMode(huh.EchoModePassword),
				huh.NewInput().Title(fmt.Sprintf("[%s] UUID (for VLESS/TUIC, blank=auto)", remark)).Value(&pUUID),
			),
		)
		if err := formP.Run(); err != nil {
			return nil, err
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
			if pPass == "" {
				if p == "ss2022" {
					pPass = randomSS2022Secret()
				} else {
					pPass = randomSecret()
				}
			}
			extra["pass"] = pPass
		}
		if p == "reality" || p == "tuic" {
			if pUUID == "" {
				pUUID = uuid.New().String()
			}
			extra["uuid"] = pUUID
		}

		// Reality extras (pbk/short will be filled after cert/key or user provided)
		if p == "reality" {
			var pbk, sid string
			formR := huh.NewForm(huh.NewGroup(
				huh.NewInput().Title("Reality public key (auto-generated during setup if empty)").Value(&pbk),
				huh.NewInput().Title("Reality shortId (8 hex; auto if empty)").Value(&sid),
			))
			_ = formR.Run() // ignore cancel
			if pbk != "" {
				extra["pbk"] = pbk
			}
			if sid == "" {
				sid = randomHex(4)
			}
			extra["short_id"] = sid
		}

		cfg.Creds[p] = extra
	}

	slog.Info("TUI collection complete", "enabled", cfg.EnabledProtocols, "port", cfg.SubPort)
	return cfg, nil
}

func randomToken() string {
	return "sub-" + randomSecret()[:16]
}

func randomSecret() string {
	b := make([]byte, 18)
	if _, err := rand.Read(b); err != nil {
		return uuid.New().String()
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
		return "01234567"
	}
	const digits = "0123456789abcdef"
	out := make([]byte, size*2)
	for i, value := range b {
		out[i*2] = digits[value>>4]
		out[i*2+1] = digits[value&0x0f]
	}
	return string(out)
}

func verifyDomainDNS(cfg *config.SetupConfig, expectedAddress string) error {
	expectedIPv4, expectedIPv6 := splitExpectedIPs(expectedAddress)
	result, err := diagnostics.CheckDomainDNS(context.Background(), cfg.Domain, expectedIPv4, expectedIPv6)
	cfg.DNSCheck = result
	if err == nil && diagnostics.DNSMatches(result) {
		return nil
	}

	description := dnsMismatchMessage(cfg.Domain, expectedIPv4, expectedIPv6, result, err)
	continueAnyway := false
	form := huh.NewForm(huh.NewGroup(
		huh.NewNote().
			Title("DNS does not point to this server yet").
			Description(description),
		huh.NewConfirm().
			Title("Continue anyway?").
			Description("ACME HTTP-01 and domain-based clients may fail until DNS is corrected.").
			Affirmative("Continue anyway").
			Negative("Abort setup").
			Value(&continueAnyway),
	))
	if formErr := form.Run(); formErr != nil {
		return formErr
	}
	if !continueAnyway {
		return fmt.Errorf("DNS for %s does not point to %s", cfg.Domain, expectedAddress)
	}
	return nil
}

func collectCertStrategy(cfg *config.SetupConfig) error {
	certMode := string(cfg.CertMode)
	if certMode == "" {
		certMode = string(config.CertStrategyCertbotHTTP)
	}
	if cfg.SetupMode == config.SetupModeIPOnly && cfg.CertMode == "" {
		certMode = string(config.CertStrategySelfSigned)
	}
	formC := huh.NewForm(
		huh.NewGroup(
			huh.NewSelect[string]().
				Title("Certificate strategy for TLS protocols").
				Description("Hysteria2, TUIC, and AnyTLS need readable cert/key files before sing-box can start.").
				Options(
					huh.NewOption("Certbot HTTP-01 (recommended, domain DNS correct, port 80 free)", string(config.CertStrategyCertbotHTTP)),
					huh.NewOption("acme.sh HTTP-01 (domain DNS correct, port 80 free)", string(config.CertStrategyACMEHTTP)),
					huh.NewOption("ACME DNS-01 Cloudflare (API token required)", string(config.CertStrategyACMEDNSCF)),
					huh.NewOption("Existing certificate files", string(config.CertStrategyExisting)),
					huh.NewOption("Self-signed / limited client compatibility", string(config.CertStrategySelfSigned)),
				).
				Value(&certMode),
		),
	)
	if err := formC.Run(); err != nil {
		return err
	}
	cfg.CertMode = config.CertStrategy(certMode)

	switch cfg.CertMode {
	case config.CertStrategyACMEDNSCF:
		formCF := huh.NewForm(huh.NewGroup(
			huh.NewInput().Title("Cloudflare API Token (DNS edit zone scope)").Value(&cfg.ACMETokenCF).EchoMode(huh.EchoModePassword),
		))
		return formCF.Run()
	case config.CertStrategyExisting:
		formExisting := huh.NewForm(huh.NewGroup(
			huh.NewInput().Title("Certificate path").Value(&cfg.CertPath).Placeholder("/etc/sub-maker/certs/fullchain.pem"),
			huh.NewInput().Title("Private key path").Value(&cfg.KeyPath).Placeholder("/etc/sub-maker/certs/privkey.pem"),
		))
		if err := formExisting.Run(); err != nil {
			return err
		}
		return diagnostics.ValidateCertificatePair(cfg.CertPath, cfg.KeyPath, cfg.Domain)
	}
	return nil
}

func splitExpectedIPs(value string) (string, string) {
	addr, err := netip.ParseAddr(value)
	if err != nil {
		return "", ""
	}
	if addr.Is4() {
		return addr.String(), ""
	}
	return "", addr.String()
}

func dnsMismatchMessage(domain, expectedIPv4, expectedIPv6 string, result *config.DNSCheckResult, lookupErr error) string {
	var b strings.Builder
	if lookupErr != nil {
		fmt.Fprintf(&b, "Lookup error: %v\n\n", lookupErr)
	}
	if expectedIPv4 != "" {
		fmt.Fprintf(&b, "Create DNS record:\n  A %s -> %s\n", domain, expectedIPv4)
	}
	if expectedIPv6 != "" {
		fmt.Fprintf(&b, "Create DNS record:\n  AAAA %s -> %s\n", domain, expectedIPv6)
	}
	if result != nil {
		if len(result.ResolvedA) > 0 {
			fmt.Fprintf(&b, "\nCurrent A records: %s", strings.Join(result.ResolvedA, ", "))
		}
		if len(result.ResolvedAAAA) > 0 {
			fmt.Fprintf(&b, "\nCurrent AAAA records: %s", strings.Join(result.ResolvedAAAA, ", "))
		}
	}
	b.WriteString("\n\nAfter DNS propagates, rerun: sudo sub-maker --setup")
	return b.String()
}

// Note: full bubbletea model for fancy progress/apply will be added in task 7 wiring.
// For now RunWizard + later Apply() in setup pkg gives working flow.
