package tui

import (
	"context"
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestDetectPublicIPUsesFirstPublicResponse(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_, _ = w.Write([]byte("8.8.8.8\n"))
	}))
	defer server.Close()

	ip, err := detectPublicIP(context.Background(), server.Client(), []publicIPEndpoint{{url: server.URL}})
	if err != nil {
		t.Fatalf("detectPublicIP returned error: %v", err)
	}
	if ip != "8.8.8.8" {
		t.Fatalf("detectPublicIP returned %q, want %q", ip, "8.8.8.8")
	}
}

func TestDetectPublicIPFallsBackAfterPrivateResponse(t *testing.T) {
	privateServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_, _ = w.Write([]byte("10.0.0.5"))
	}))
	defer privateServer.Close()

	publicServer := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_, _ = w.Write([]byte("1.1.1.1"))
	}))
	defer publicServer.Close()

	ip, err := detectPublicIP(context.Background(), privateServer.Client(), []publicIPEndpoint{{url: privateServer.URL}, {url: publicServer.URL}})
	if err != nil {
		t.Fatalf("detectPublicIP returned error: %v", err)
	}
	if ip != "1.1.1.1" {
		t.Fatalf("detectPublicIP returned %q, want %q", ip, "1.1.1.1")
	}
}

func TestIsPublicIPRejectsNonPublicValues(t *testing.T) {
	for _, value := range []string{"", "not-an-ip", "127.0.0.1", "10.0.0.1", "192.168.1.1", "::1", "fc00::1"} {
		if isPublicIP(value) {
			t.Fatalf("isPublicIP(%q) = true, want false", value)
		}
	}
}
