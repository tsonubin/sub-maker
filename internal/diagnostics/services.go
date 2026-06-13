package diagnostics

import (
	"fmt"
	"io"
	"net/http"
	"os/exec"
	"strings"
	"time"
)

func RunSystemctl(args ...string) error {
	cmd := exec.Command("systemctl", args...)
	out, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("systemctl %s: %w\n%s", strings.Join(args, " "), err, strings.TrimSpace(string(out)))
	}
	return nil
}

func SystemctlOutput(args ...string) (string, error) {
	cmd := exec.Command("systemctl", args...)
	out, err := cmd.CombinedOutput()
	if err != nil {
		return string(out), fmt.Errorf("systemctl %s: %w", strings.Join(args, " "), err)
	}
	return string(out), nil
}

func ServiceActive(service string) error {
	return RunSystemctl("is-active", "--quiet", service)
}

func VerifySubscriptionEndpoint(url string) error {
	client := &http.Client{Timeout: 5 * time.Second}
	resp, err := client.Get(url)
	if err != nil {
		return fmt.Errorf("request subscription endpoint: %w", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("subscription endpoint returned %s", resp.Status)
	}
	body, err := io.ReadAll(io.LimitReader(resp.Body, 64*1024))
	if err != nil {
		return fmt.Errorf("read subscription response: %w", err)
	}
	if !strings.Contains(string(body), "proxy-groups:") {
		return fmt.Errorf("subscription endpoint did not return Clash YAML")
	}
	return nil
}
