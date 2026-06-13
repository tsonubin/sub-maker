package setup

import (
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/pem"
	"fmt"
	"log/slog"
	"math/big"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"time"

	"github.com/tsonubin/sub-maker/internal/config"
)

// EnsureAcme installs acme.sh if missing and issues cert for domain using the chosen mode.
// This is called from TUI or Apply.
func EnsureAcme(domain, email string, mode config.CertStrategy, cfToken string) error {
	if domain == "" {
		return fmt.Errorf("domain required for real cert")
	}
	// install acme.sh if not present
	if _, err := exec.LookPath("acme.sh"); err != nil {
		slog.Info("installing acme.sh ...")
		cmd := exec.Command("sh", "-c", "curl https://get.acme.sh | sh -s email="+email)
		if out, err := cmd.CombinedOutput(); err != nil {
			return fmt.Errorf("acme install: %s %w", string(out), err)
		}
	}

	acme := filepath.Join(acmeHome(), "acme.sh")
	if mode == config.CertStrategyACMEDNSCF && cfToken != "" {
		cmd := exec.Command(acme, "--issue", "--dns", "dns_cf", "-d", domain, "--keylength", "ec-256")
		cmd.Env = append(os.Environ(), "CF_Token="+cfToken)
		out, err := cmd.CombinedOutput()
		slog.Info("acme dns output", "out", string(out))
		if err != nil {
			return fmt.Errorf("acme dns: %w", err)
		}
		return nil
	}
	// default http-01 (standalone, needs port 80)
	cmd := exec.Command(acme, "--issue", "--standalone", "-d", domain, "--keylength", "ec-256")
	out, err := cmd.CombinedOutput()
	slog.Info("acme http output", "out", string(out))
	if err != nil {
		return fmt.Errorf("acme http: %w", err)
	}
	return nil
}

func EnsureCertbot(domain, email string) error {
	if domain == "" {
		return fmt.Errorf("domain required for certbot")
	}
	if _, err := exec.LookPath("certbot"); err != nil {
		if err := installCertbot(); err != nil {
			return err
		}
	}

	args := []string{"certonly", "--standalone", "-d", domain, "--agree-tos", "--non-interactive"}
	if email != "" {
		args = append(args, "--email", email)
	} else {
		args = append(args, "--register-unsafely-without-email")
	}
	cmd := exec.Command("certbot", args...)
	out, err := cmd.CombinedOutput()
	slog.Info("certbot output", "out", string(out))
	if err != nil {
		return fmt.Errorf("certbot issue: %w\n%s", err, string(out))
	}
	return nil
}

func installCertbot() error {
	slog.Info("installing certbot")
	var cmd *exec.Cmd
	switch {
	case commandExists("apt-get"):
		cmd = exec.Command("sh", "-c", "apt-get update -qq && apt-get install -y -qq certbot")
	case commandExists("dnf"):
		cmd = exec.Command("dnf", "install", "-y", "certbot")
	case commandExists("yum"):
		cmd = exec.Command("yum", "install", "-y", "certbot")
	case commandExists("apk"):
		cmd = exec.Command("apk", "add", "--no-cache", "certbot")
	default:
		return fmt.Errorf("certbot not found and no supported package manager was detected")
	}
	out, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("install certbot: %w\n%s", err, string(out))
	}
	return nil
}

// CopyCertsToEtc copies the issued certs to /etc/sub-maker/certs for sing-box use.
func CopyCertsToEtc(domain string) error {
	src := filepath.Join(acmeHome(), domain+"_ecc")
	if _, err := os.Stat(src); err != nil {
		src = filepath.Join(acmeHome(), domain)
	}
	dst := "/etc/sub-maker/certs"
	if err := os.MkdirAll(dst, 0755); err != nil {
		return err
	}
	if err := copyFile(filepath.Join(src, "fullchain.cer"), filepath.Join(dst, "fullchain.pem"), 0644); err != nil {
		return err
	}
	if err := copyFile(filepath.Join(src, domain+".key"), filepath.Join(dst, "privkey.pem"), 0600); err != nil {
		return err
	}
	slog.Info("certs copied", "dst", dst)
	return nil
}

func CopyCertbotCertsToEtc(domain string) error {
	src := filepath.Join("/etc/letsencrypt/live", domain)
	dst := "/etc/sub-maker/certs"
	if err := os.MkdirAll(dst, 0755); err != nil {
		return err
	}
	if err := copyFile(filepath.Join(src, "fullchain.pem"), filepath.Join(dst, "fullchain.pem"), 0644); err != nil {
		return err
	}
	if err := copyFile(filepath.Join(src, "privkey.pem"), filepath.Join(dst, "privkey.pem"), 0600); err != nil {
		return err
	}
	slog.Info("certbot certs copied", "dst", dst)
	return nil
}

func GenerateSelfSignedCert(domain, serverAddr string) error {
	dst := "/etc/sub-maker/certs"
	if os.Getenv("SUB_MAKER_DEMO") != "" {
		dst = "/tmp/sub-maker-demo-etc/sub-maker/certs"
	}
	if err := os.MkdirAll(dst, 0755); err != nil {
		return err
	}
	key, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		return err
	}
	serial, err := rand.Int(rand.Reader, new(big.Int).Lsh(big.NewInt(1), 128))
	if err != nil {
		return err
	}
	tpl := x509.Certificate{
		SerialNumber: serial,
		Subject:      pkix.Name{CommonName: domain},
		NotBefore:    time.Now().Add(-time.Hour),
		NotAfter:     time.Now().AddDate(1, 0, 0),
		KeyUsage:     x509.KeyUsageDigitalSignature | x509.KeyUsageKeyEncipherment,
		ExtKeyUsage:  []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
		DNSNames:     []string{},
	}
	if domain != "" {
		tpl.DNSNames = append(tpl.DNSNames, domain)
	}
	if ip := net.ParseIP(serverAddr); ip != nil {
		tpl.IPAddresses = append(tpl.IPAddresses, ip)
	}
	certDER, err := x509.CreateCertificate(rand.Reader, &tpl, &tpl, &key.PublicKey, key)
	if err != nil {
		return err
	}
	certPath := filepath.Join(dst, "fullchain.pem")
	keyPath := filepath.Join(dst, "privkey.pem")
	if err := writePEM(certPath, 0644, "CERTIFICATE", certDER); err != nil {
		return err
	}
	if err := writePEM(keyPath, 0600, "RSA PRIVATE KEY", x509.MarshalPKCS1PrivateKey(key)); err != nil {
		return err
	}
	return nil
}

func commandExists(name string) bool {
	_, err := exec.LookPath(name)
	return err == nil
}

func acmeHome() string {
	home := "/root"
	if h := os.Getenv("HOME"); h != "" {
		home = h
	}
	return filepath.Join(home, ".acme.sh")
}

func copyFile(src, dst string, mode os.FileMode) error {
	data, err := os.ReadFile(src)
	if err != nil {
		return err
	}
	return os.WriteFile(dst, data, mode)
}

func writePEM(path string, mode os.FileMode, blockType string, bytes []byte) error {
	f, err := os.OpenFile(path, os.O_CREATE|os.O_TRUNC|os.O_WRONLY, mode)
	if err != nil {
		return err
	}
	defer f.Close()
	return pem.Encode(f, &pem.Block{Type: blockType, Bytes: bytes})
}
