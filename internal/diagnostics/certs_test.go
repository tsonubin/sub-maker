package diagnostics

import (
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/pem"
	"math/big"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestProtocolsRequireLocalCert(t *testing.T) {
	if ProtocolsRequireLocalCert([]string{"reality", "ss2022"}) {
		t.Fatal("reality and ss2022 should not require local certs")
	}
	if !ProtocolsRequireLocalCert([]string{"reality", "hysteria2"}) {
		t.Fatal("hysteria2 should require local certs")
	}
}

func TestValidateCertificatePairAcceptsMatchingCert(t *testing.T) {
	certPath, keyPath := writeTestCert(t, "proxy.example.com", time.Now().Add(-time.Hour), time.Now().Add(time.Hour))

	if err := ValidateCertificatePair(certPath, keyPath, "proxy.example.com"); err != nil {
		t.Fatalf("ValidateCertificatePair returned error: %v", err)
	}
}

func TestValidateCertificatePairRejectsWrongDomain(t *testing.T) {
	certPath, keyPath := writeTestCert(t, "proxy.example.com", time.Now().Add(-time.Hour), time.Now().Add(time.Hour))

	if err := ValidateCertificatePair(certPath, keyPath, "other.example.com"); err == nil {
		t.Fatal("expected wrong-domain error")
	}
}

func writeTestCert(t *testing.T, domain string, notBefore, notAfter time.Time) (string, string) {
	t.Helper()

	key, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		t.Fatalf("generate key: %v", err)
	}
	template := x509.Certificate{
		SerialNumber: big.NewInt(1),
		Subject:      pkix.Name{CommonName: domain},
		DNSNames:     []string{domain},
		NotBefore:    notBefore,
		NotAfter:     notAfter,
		KeyUsage:     x509.KeyUsageKeyEncipherment | x509.KeyUsageDigitalSignature,
		ExtKeyUsage:  []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
	}
	certDER, err := x509.CreateCertificate(rand.Reader, &template, &template, &key.PublicKey, key)
	if err != nil {
		t.Fatalf("create cert: %v", err)
	}

	dir := t.TempDir()
	certPath := filepath.Join(dir, "fullchain.pem")
	keyPath := filepath.Join(dir, "privkey.pem")
	certFile, err := os.Create(certPath)
	if err != nil {
		t.Fatalf("create cert file: %v", err)
	}
	if err := pem.Encode(certFile, &pem.Block{Type: "CERTIFICATE", Bytes: certDER}); err != nil {
		t.Fatalf("write cert: %v", err)
	}
	if err := certFile.Close(); err != nil {
		t.Fatalf("close cert: %v", err)
	}

	keyFile, err := os.Create(keyPath)
	if err != nil {
		t.Fatalf("create key file: %v", err)
	}
	if err := pem.Encode(keyFile, &pem.Block{Type: "RSA PRIVATE KEY", Bytes: x509.MarshalPKCS1PrivateKey(key)}); err != nil {
		t.Fatalf("write key: %v", err)
	}
	if err := keyFile.Close(); err != nil {
		t.Fatalf("close key: %v", err)
	}
	return certPath, keyPath
}
