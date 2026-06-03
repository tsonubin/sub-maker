package setup

import (
	"fmt"
	"log/slog"
	"os"
	"os/exec"
)

// EnsureAcme installs acme.sh if missing and issues cert for domain using the chosen mode.
// This is called from TUI or Apply.
func EnsureAcme(domain, email, mode, cfToken string) error {
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

	acme := "$HOME/.acme.sh/acme.sh"
	if mode == "acme-dns-cf" && cfToken != "" {
		// export for the shell
		cmd := exec.Command("sh", "-c", fmt.Sprintf("export CF_Token=%s; %s --issue --dns dns_cf -d %s --keylength ec-256", cfToken, acme, domain))
		out, err := cmd.CombinedOutput()
		slog.Info("acme dns output", "out", string(out))
		return err
	}
	// default http-01 (standalone, needs port 80)
	cmd := exec.Command("sh", "-c", fmt.Sprintf("%s --issue --standalone -d %s --keylength ec-256", acme, domain))
	out, err := cmd.CombinedOutput()
	slog.Info("acme http output", "out", string(out))
	return err
}

// CopyCertsToEtc copies the issued certs to /etc/sub-maker/certs for sing-box use.
func CopyCertsToEtc(domain string) error {
	home := "/root"
	if h := os.Getenv("HOME"); h != "" {
		home = h
	}
	src := fmt.Sprintf("%s/.acme.sh/%s", home, domain)
	dst := "/etc/sub-maker/certs"
	os.MkdirAll(dst, 0755)
	// copy fullchain + key (ec or rsa)
	for _, f := range []string{"fullchain.cer", "ca.cer", domain+".key"} {
		// simplistic cp
		exec.Command("cp", "-f", src+"/"+f, dst+"/"+f).Run()
	}
	// also link privkey
	exec.Command("cp", "-f", src+"/"+domain+".key", dst+"/privkey.pem").Run()
	exec.Command("cp", "-f", src+"/fullchain.cer", dst+"/fullchain.pem").Run()
	slog.Info("certs copied", "dst", dst)
	return nil
}
