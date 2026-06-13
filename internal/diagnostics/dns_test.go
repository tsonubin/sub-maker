package diagnostics

import (
	"context"
	"errors"
	"net"
	"testing"
)

type fakeResolver struct {
	addrs []net.IPAddr
	err   error
}

func (f fakeResolver) LookupIPAddr(context.Context, string) ([]net.IPAddr, error) {
	return f.addrs, f.err
}

func TestCheckDomainDNSMatchesIPv4(t *testing.T) {
	result, err := checkDomainDNS(context.Background(), fakeResolver{
		addrs: []net.IPAddr{{IP: net.ParseIP("43.245.198.178")}},
	}, "example.com", "43.245.198.178", "")
	if err != nil {
		t.Fatalf("checkDomainDNS returned error: %v", err)
	}
	if !result.IPv4Matches || !DNSMatches(result) {
		t.Fatalf("expected DNS match, got %#v", result)
	}
}

func TestCheckDomainDNSDetectsMismatch(t *testing.T) {
	result, err := checkDomainDNS(context.Background(), fakeResolver{
		addrs: []net.IPAddr{{IP: net.ParseIP("1.1.1.1")}},
	}, "example.com", "43.245.198.178", "")
	if err != nil {
		t.Fatalf("checkDomainDNS returned error: %v", err)
	}
	if result.IPv4Matches || DNSMatches(result) {
		t.Fatalf("expected DNS mismatch, got %#v", result)
	}
}

func TestCheckDomainDNSReturnsPartialResultOnLookupError(t *testing.T) {
	result, err := checkDomainDNS(context.Background(), fakeResolver{
		err: errors.New("no such host"),
	}, "example.com", "43.245.198.178", "")
	if err == nil {
		t.Fatal("expected lookup error")
	}
	if result == nil || result.Domain != "example.com" {
		t.Fatalf("expected partial result, got %#v", result)
	}
}
