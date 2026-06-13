package tui

import (
	"context"
	"fmt"
	"log/slog"
	"os"
	"strings"

	"github.com/charmbracelet/huh"
	"github.com/tsonubin/sub-maker/internal/config"
)

// RunWizard runs the full interactive collection using huh (simple yet powerful forms).
// Returns the populated config (ready for Apply).
// This is the main entry from main --setup.
func RunWizard() (*config.SetupConfig, error) {
	cfg := config.DefaultConfig()

	// 1. Welcome + global info
	var serverAddr, domain, email, token, subPortStr string
	var subPort int
	if detectedIP, err := DetectPublicIP(context.Background()); err == nil {
		serverAddr = detectedIP
		slog.Info("detected server public IP", "ip", detectedIP)
	} else {
		slog.Info("could not auto-detect server public IP", "err", err)
	}

	form := huh.NewForm(
		huh.NewGroup(
			huh.NewNote().
				Title("sub-maker TUI").
				Description("Setup top-5 GFW-resistant transports (VLESS-Reality, Hysteria2, TUICv5, AnyTLS, SS2022)\n+ Clash subscription with common ACL4SSR rulesets via subconverter on :8964."),
			huh.NewInput().Title("Server public address (IP or domain for client links)").Value(&serverAddr).Placeholder("1.2.3.4 or your.domain.com"),
			huh.NewInput().Title("Domain for ACME certs + default SNI (recommended for real certs)").Value(&domain).Placeholder("your.domain.com"),
			huh.NewInput().Title("ACME contact email").Value(&email).Placeholder("admin@example.com"),
			huh.NewInput().Title("Subscription token (auto-generated if empty)").Value(&token).Placeholder("leave blank to auto-gen"),
			huh.NewInput().Title("Subscription listen port").Value(&subPortStr).Placeholder("8964"),
		),
	)
	if err := form.Run(); err != nil {
		return nil, err
	}

	if serverAddr == "" {
		serverAddr = "127.0.0.1"
	}
	if token == "" {
		token = randomToken() // simple impl below or from generator
	}
	if subPortStr != "" {
		fmt.Sscanf(subPortStr, "%d", &subPort)
	}
	if subPort == 0 {
		subPort = 8964
	}

	cfg.ServerAddr = serverAddr
	cfg.Domain = domain
	cfg.Email = email
	cfg.SubToken = token
	cfg.SubPort = subPort

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

	// 3. Per-protocol details (ports, creds, remarks) - sequential for simplicity
	for _, p := range selected {
		port := cfg.Ports[p]
		remark := strings.Title(p)
		extra := map[string]string{}

		portStr := fmt.Sprintf("%d", port)
		var pRemark, pSNI, pPass, pUUID string

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
		if pPass != "" {
			extra["pass"] = pPass
		}
		if pUUID != "" {
			extra["uuid"] = pUUID
		}

		// Reality extras (pbk/short will be filled after cert/key or user provided)
		if p == "reality" {
			var pbk, sid string
			formR := huh.NewForm(huh.NewGroup(
				huh.NewInput().Title("Reality public key (pbk; generate later if empty)").Value(&pbk),
				huh.NewInput().Title("Reality shortId (8 hex; auto if empty)").Value(&sid),
			))
			_ = formR.Run() // ignore cancel
			if pbk != "" {
				extra["pbk"] = pbk
			}
			if sid != "" {
				extra["short_id"] = sid
			}
		}

		cfg.Creds[p] = extra
	}

	// 4. Cert mode
	var certMode string
	cfToken := ""
	formC := huh.NewForm(
		huh.NewGroup(
			huh.NewSelect[string]().
				Title("Cert strategy (acme.sh recommended for real certs)").
				Options(
					huh.NewOption("ACME HTTP-01 (port 80 must be free)", "acme-http"),
					huh.NewOption("ACME DNS-01 Cloudflare (paste token next)", "acme-dns-cf"),
					huh.NewOption("Self-signed (Reality works without; others will be insecure)", "self-signed"),
				).
				Value(&certMode),
		),
	)
	if err := formC.Run(); err != nil {
		return nil, err
	}
	cfg.CertMode = certMode

	if certMode == "acme-dns-cf" {
		formCF := huh.NewForm(huh.NewGroup(
			huh.NewInput().Title("Cloudflare API Token (DNS edit zone scope)").Value(&cfToken).EchoMode(huh.EchoModePassword),
		))
		_ = formCF.Run()
		cfg.ACMETokenCF = cfToken
	}

	slog.Info("TUI collection complete", "enabled", cfg.EnabledProtocols, "port", cfg.SubPort)
	return cfg, nil
}

func randomToken() string {
	// simple; real uses generator or crypto/rand
	b := make([]byte, 12)
	_, _ = os.ReadFile("/dev/urandom") // best effort
	if len(b) == 0 {
		b = []byte{1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12}
	}
	return "sub-" + fmt.Sprintf("%x", b)[:12]
}

// Note: full bubbletea model for fancy progress/apply will be added in task 7 wiring.
// For now RunWizard + later Apply() in setup pkg gives working flow.
