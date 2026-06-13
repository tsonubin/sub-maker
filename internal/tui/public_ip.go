package tui

import (
	"context"
	"fmt"
	"io"
	"net/http"
	"net/netip"
	"strings"
	"time"
)

var publicIPEndpoints = []string{
	"https://api.ipify.org",
	"https://ifconfig.me/ip",
	"https://icanhazip.com",
}

// DetectPublicIP returns the server's outward-facing public IP address.
func DetectPublicIP(ctx context.Context) (string, error) {
	ctx, cancel := context.WithTimeout(ctx, 3*time.Second)
	defer cancel()

	client := &http.Client{Timeout: 1500 * time.Millisecond}
	return detectPublicIP(ctx, client, publicIPEndpoints)
}

func detectPublicIP(ctx context.Context, client *http.Client, endpoints []string) (string, error) {
	var lastErr error
	for _, endpoint := range endpoints {
		ip, err := fetchPublicIP(ctx, client, endpoint)
		if err == nil {
			return ip, nil
		}
		lastErr = err
	}
	if lastErr == nil {
		lastErr = fmt.Errorf("no public IP endpoints configured")
	}
	return "", lastErr
}

func fetchPublicIP(ctx context.Context, client *http.Client, endpoint string) (string, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, endpoint, nil)
	if err != nil {
		return "", err
	}

	resp, err := client.Do(req)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()

	if resp.StatusCode < http.StatusOK || resp.StatusCode >= http.StatusMultipleChoices {
		return "", fmt.Errorf("%s returned %s", endpoint, resp.Status)
	}

	body, err := io.ReadAll(io.LimitReader(resp.Body, 128))
	if err != nil {
		return "", err
	}
	ip := strings.TrimSpace(string(body))
	if !isPublicIP(ip) {
		return "", fmt.Errorf("%s returned non-public IP %q", endpoint, ip)
	}
	return ip, nil
}

func isPublicIP(value string) bool {
	addr, err := netip.ParseAddr(value)
	if err != nil {
		return false
	}
	return addr.IsGlobalUnicast() &&
		!addr.IsPrivate() &&
		!addr.IsLoopback() &&
		!addr.IsLinkLocalUnicast() &&
		!addr.IsUnspecified()
}
