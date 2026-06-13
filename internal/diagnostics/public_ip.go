package diagnostics

import (
	"context"
	"fmt"
	"io"
	"net/http"
	"net/netip"
	"strings"
	"time"
)

type publicIPEndpoint struct {
	url     string
	headers map[string]string
}

var publicIPEndpoints = []publicIPEndpoint{
	{url: "https://api.ipify.org"},
	{url: "https://ifconfig.me/ip"},
	{url: "https://icanhazip.com"},
	{url: "http://169.254.169.254/latest/meta-data/public-ipv4"},
	{url: "http://169.254.169.254/metadata/v1/interfaces/public/0/ipv4/address"},
	{url: "http://metadata.google.internal/computeMetadata/v1/instance/network-interfaces/0/access-configs/0/external-ip", headers: map[string]string{"Metadata-Flavor": "Google"}},
	{url: "http://169.254.169.254/metadata/instance/network/interface/0/ipv4/ipAddress/0/publicIpAddress?api-version=2021-02-01&format=text", headers: map[string]string{"Metadata": "true"}},
}

// DetectPublicIP returns the server's outward-facing public IP address.
func DetectPublicIP(ctx context.Context) (string, error) {
	ctx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()

	client := &http.Client{Timeout: 1500 * time.Millisecond}
	return detectPublicIP(ctx, client, publicIPEndpoints)
}

func detectPublicIP(ctx context.Context, client *http.Client, endpoints []publicIPEndpoint) (string, error) {
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

func fetchPublicIP(ctx context.Context, client *http.Client, endpoint publicIPEndpoint) (string, error) {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, endpoint.url, nil)
	if err != nil {
		return "", err
	}
	for key, value := range endpoint.headers {
		req.Header.Set(key, value)
	}

	resp, err := client.Do(req)
	if err != nil {
		return "", err
	}
	defer resp.Body.Close()

	if resp.StatusCode < http.StatusOK || resp.StatusCode >= http.StatusMultipleChoices {
		return "", fmt.Errorf("%s returned %s", endpoint.url, resp.Status)
	}

	body, err := io.ReadAll(io.LimitReader(resp.Body, 128))
	if err != nil {
		return "", err
	}
	ip := strings.TrimSpace(string(body))
	if !isPublicIP(ip) {
		return "", fmt.Errorf("%s returned non-public IP %q", endpoint.url, ip)
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
