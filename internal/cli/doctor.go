package cli

import (
	"context"
	"fmt"
	"os"

	"github.com/tsonubin/sub-maker/internal/diagnostics"
	"github.com/tsonubin/sub-maker/internal/setup"
)

func Doctor() error {
	cfg, err := LoadConfig()
	if err != nil {
		return fmt.Errorf("load config %s: %w", ConfigPath(), err)
	}

	ok := true
	check := func(name string, err error) {
		if err != nil {
			ok = false
			fmt.Printf("[FAIL] %s: %v\n", name, err)
			return
		}
		fmt.Printf("[ OK ] %s\n", name)
	}

	check("config file", nil)
	nodes, err := os.ReadFile(NodesPath())
	if err == nil && len(nodes) == 0 {
		err = fmt.Errorf("nodes file is empty")
	}
	check("nodes file", err)

	publicIP, err := diagnostics.DetectPublicIP(context.Background())
	if err == nil {
		fmt.Printf("[INFO] detected public IP: %s\n", publicIP)
	}
	if cfg.Domain != "" {
		expectedIPv4, expectedIPv6 := diagnostics.SplitExpectedIPs(publicIP)
		if publicIP == "" {
			expectedIPv4, expectedIPv6 = diagnostics.SplitExpectedIPs(cfg.ServerAddr)
		}
		result, dnsErr := diagnostics.CheckDomainDNS(context.Background(), cfg.Domain, expectedIPv4, expectedIPv6)
		if dnsErr == nil && !diagnostics.DNSMatches(result) {
			dnsErr = fmt.Errorf("resolved A=%v AAAA=%v, expected %s %s", result.ResolvedA, result.ResolvedAAAA, expectedIPv4, expectedIPv6)
		}
		check("domain DNS", dnsErr)
	}

	if diagnostics.ProtocolsRequireLocalCert(cfg.EnabledProtocols) {
		check("certificate files", diagnostics.ValidateCertificatePair(cfg.CertPath, cfg.KeyPath, cfg.Domain))
	}

	for _, service := range managedServices {
		if service == "subconverter" && !setup.SubconverterInstalled() {
			fmt.Printf("[SKIP] service subconverter: binary not installed\n")
			continue
		}
		check("service "+service, diagnostics.ServiceActive(service))
	}

	localURL := fmt.Sprintf("http://127.0.0.1:%d/sub?token=%s", cfg.SubPort, cfg.SubToken)
	check("local subscription endpoint", diagnostics.VerifySubscriptionEndpoint(localURL))

	if err := PrintLinks(); err != nil {
		check("links", err)
	}
	if !ok {
		return fmt.Errorf("doctor found failing checks")
	}
	return nil
}
