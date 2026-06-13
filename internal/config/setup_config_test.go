package config

import (
	"strings"
	"testing"

	"gopkg.in/yaml.v3"
)

func TestLinkPasscodeHashVerifiesOnlyCorrectPasscode(t *testing.T) {
	cfg := DefaultConfig()
	if err := cfg.SetLinkPasscode("123456"); err != nil {
		t.Fatalf("SetLinkPasscode returned error: %v", err)
	}
	if cfg.LinkPasscodeHash == "" || cfg.LinkPasscodeSalt == "" {
		t.Fatal("expected passcode hash and salt")
	}
	if cfg.LinkPasscodeHash == "123456" {
		t.Fatal("passcode must not be stored in plaintext")
	}
	if !cfg.VerifyLinkPasscode("123456") {
		t.Fatal("expected correct passcode to verify")
	}
	if cfg.VerifyLinkPasscode("654321") {
		t.Fatal("expected wrong passcode to fail")
	}
}

func TestGenerateLinkPasscodeReturnsHumanSizedCode(t *testing.T) {
	passcode := GenerateLinkPasscode()
	if len(passcode) != 10 {
		t.Fatalf("expected 10-character passcode, got %q", passcode)
	}
}

func TestLinkPasscodePlainIsNotSerialized(t *testing.T) {
	cfg := DefaultConfig()
	if err := cfg.SetLinkPasscode("do-not-store"); err != nil {
		t.Fatalf("SetLinkPasscode returned error: %v", err)
	}
	data, err := yaml.Marshal(cfg)
	if err != nil {
		t.Fatalf("marshal config: %v", err)
	}
	if strings.Contains(string(data), "do-not-store") {
		t.Fatalf("plaintext passcode was serialized:\n%s", string(data))
	}
}
