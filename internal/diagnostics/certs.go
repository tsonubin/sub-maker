package diagnostics

import (
	"crypto/x509"
	"encoding/pem"
	"fmt"
	"os"
	"time"
)

func ProtocolsRequireLocalCert(protocols []string) bool {
	for _, protocol := range protocols {
		switch protocol {
		case "hysteria2", "tuic", "anytls":
			return true
		}
	}
	return false
}

func ValidateCertificatePair(certPath, keyPath, domain string) error {
	certPEM, err := os.ReadFile(certPath)
	if err != nil {
		return fmt.Errorf("read certificate: %w", err)
	}
	if _, err := os.Stat(keyPath); err != nil {
		return fmt.Errorf("read private key: %w", err)
	}

	block, _ := pem.Decode(certPEM)
	if block == nil {
		return fmt.Errorf("certificate file does not contain PEM data")
	}
	cert, err := x509.ParseCertificate(block.Bytes)
	if err != nil {
		return fmt.Errorf("parse certificate: %w", err)
	}

	now := time.Now()
	if now.Before(cert.NotBefore) {
		return fmt.Errorf("certificate is not valid before %s", cert.NotBefore.Format(time.RFC3339))
	}
	if now.After(cert.NotAfter) {
		return fmt.Errorf("certificate expired at %s", cert.NotAfter.Format(time.RFC3339))
	}
	if domain != "" {
		if err := cert.VerifyHostname(domain); err != nil {
			return fmt.Errorf("certificate does not cover %s: %w", domain, err)
		}
	}
	return nil
}
