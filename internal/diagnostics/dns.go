package diagnostics

import (
	"context"
	"fmt"
	"net"
	"net/netip"
	"sort"
	"time"

	"github.com/tsonubin/sub-maker/internal/config"
)

type ipResolver interface {
	LookupIPAddr(ctx context.Context, host string) ([]net.IPAddr, error)
}

type defaultResolver struct{}

func (defaultResolver) LookupIPAddr(ctx context.Context, host string) ([]net.IPAddr, error) {
	return net.DefaultResolver.LookupIPAddr(ctx, host)
}

func CheckDomainDNS(ctx context.Context, domain, expectedIPv4, expectedIPv6 string) (*config.DNSCheckResult, error) {
	return checkDomainDNS(ctx, defaultResolver{}, domain, expectedIPv4, expectedIPv6)
}

func checkDomainDNS(ctx context.Context, resolver ipResolver, domain, expectedIPv4, expectedIPv6 string) (*config.DNSCheckResult, error) {
	if domain == "" {
		return nil, fmt.Errorf("domain is required")
	}

	addrs, err := resolver.LookupIPAddr(ctx, domain)
	result := &config.DNSCheckResult{
		Domain:        domain,
		ExpectedIPv4:  expectedIPv4,
		ExpectedIPv6:  expectedIPv6,
		LastCheckedAt: time.Now().UTC().Format(time.RFC3339),
	}
	if err != nil {
		return result, err
	}

	for _, ipAddr := range addrs {
		addr, ok := netip.AddrFromSlice(ipAddr.IP)
		if !ok {
			continue
		}
		addr = addr.Unmap()
		switch {
		case addr.Is4():
			result.ResolvedA = append(result.ResolvedA, addr.String())
		case addr.Is6():
			result.ResolvedAAAA = append(result.ResolvedAAAA, addr.String())
		}
	}
	sort.Strings(result.ResolvedA)
	sort.Strings(result.ResolvedAAAA)

	result.IPv4Matches = expectedIPv4 == "" || containsIP(result.ResolvedA, expectedIPv4)
	result.IPv6Matches = expectedIPv6 == "" || containsIP(result.ResolvedAAAA, expectedIPv6)
	return result, nil
}

func DNSMatches(result *config.DNSCheckResult) bool {
	if result == nil {
		return false
	}
	return result.IPv4Matches && result.IPv6Matches
}

func containsIP(values []string, expected string) bool {
	if expected == "" {
		return true
	}
	addr, err := netip.ParseAddr(expected)
	if err != nil {
		return false
	}
	expected = addr.Unmap().String()
	for _, value := range values {
		got, err := netip.ParseAddr(value)
		if err == nil && got.Unmap().String() == expected {
			return true
		}
	}
	return false
}
