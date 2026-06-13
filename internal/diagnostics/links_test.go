package diagnostics

import (
	"testing"

	"github.com/tsonubin/sub-maker/internal/config"
)

func TestBuildLinksPrefersDomain(t *testing.T) {
	links := BuildLinks(&config.SetupConfig{
		ServerAddr: "43.245.198.178",
		Domain:     "proxy.example.com",
		SubPort:    8964,
		SubToken:   "tok",
	})
	if links.Subscription != "http://proxy.example.com:8964/sub?token=tok" {
		t.Fatalf("unexpected subscription link: %s", links.Subscription)
	}
	if links.IPFallback != "http://43.245.198.178:8964/sub?token=tok" {
		t.Fatalf("unexpected fallback link: %s", links.IPFallback)
	}
}

func TestBuildLinksFallsBackToIP(t *testing.T) {
	links := BuildLinks(&config.SetupConfig{
		ServerAddr: "43.245.198.178",
		SubPort:    8964,
		SubToken:   "tok",
	})
	if links.Subscription != "http://43.245.198.178:8964/sub?token=tok" {
		t.Fatalf("unexpected subscription link: %s", links.Subscription)
	}
}
