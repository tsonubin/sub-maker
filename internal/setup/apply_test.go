package setup

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/tsonubin/sub-maker/internal/config"
)

func TestParseRealityKeypairOutputAcceptsSingBoxLabels(t *testing.T) {
	output := "PrivateKey: kLdPxlfZRBqpLh0Pf4C3JvVNvtHdCMCGnAjx2vA3KXU\nPublicKey: ghEQYzsLid4rCIyA694d399DIkYPuohND5NoqILIzSs\n"

	privateKey, publicKey, err := parseRealityKeypairOutput(output)
	if err != nil {
		t.Fatalf("parseRealityKeypairOutput returned error: %v", err)
	}
	if privateKey != "kLdPxlfZRBqpLh0Pf4C3JvVNvtHdCMCGnAjx2vA3KXU" {
		t.Fatalf("unexpected private key: %s", privateKey)
	}
	if publicKey != "ghEQYzsLid4rCIyA694d399DIkYPuohND5NoqILIzSs" {
		t.Fatalf("unexpected public key: %s", publicKey)
	}
}

func TestParseRealityKeypairOutputAcceptsSeparatedLabels(t *testing.T) {
	output := "Private key: private_value\nPublic key: public_value\n"

	privateKey, publicKey, err := parseRealityKeypairOutput(output)
	if err != nil {
		t.Fatalf("parseRealityKeypairOutput returned error: %v", err)
	}
	if privateKey != "private_value" || publicKey != "public_value" {
		t.Fatalf("unexpected keys: private=%s public=%s", privateKey, publicKey)
	}
}

func TestProtocolServerNameUsesRealityCamouflageSNI(t *testing.T) {
	cfg := &config.SetupConfig{Domain: "proxy.example.com"}
	creds := map[string]string{"sni": "www.apple.com"}

	if got := protocolServerName("reality", cfg, creds); got != "www.apple.com" {
		t.Fatalf("expected camouflage sni, got %q", got)
	}
}

func TestProtocolServerNameDefaultsRealityWithoutOverride(t *testing.T) {
	cfg := &config.SetupConfig{Domain: "proxy.example.com"}

	if got := protocolServerName("reality", cfg, map[string]string{}); got != "www.apple.com" {
		t.Fatalf("expected default reality sni, got %q", got)
	}
}

func TestProtocolServerNameUsesDomainForTLSProtocols(t *testing.T) {
	cfg := &config.SetupConfig{Domain: "proxy.example.com"}

	if got := protocolServerName("hysteria2", cfg, map[string]string{}); got != "proxy.example.com" {
		t.Fatalf("expected domain sni, got %q", got)
	}
}

func TestWriteSystemdUnitsRendersSubMakerSubscriptionUnit(t *testing.T) {
	t.Setenv("SUB_MAKER_DEMO", "1")
	t.Chdir(t.TempDir())
	unitPath := filepath.Join("/tmp", "sub-maker-sub.service")
	t.Cleanup(func() {
		_ = os.Remove(unitPath)
	})
	_ = os.Remove(unitPath)

	cfg := &config.SetupConfig{
		SubToken: "test-token",
		SubPort:  1984,
	}
	nodesPath := "/etc/sub-maker/nodes.txt"

	if err := writeSystemdUnits(cfg, nodesPath); err != nil {
		t.Fatalf("writeSystemdUnits returned error: %v", err)
	}

	data, err := os.ReadFile(unitPath)
	if err != nil {
		t.Fatalf("read rendered unit: %v", err)
	}
	unit := string(data)
	for _, want := range []string{
		"Environment=SUB_MAKER_TOKEN=test-token",
		"Environment=SUB_MAKER_PORT=1984",
		"Environment=SUB_MAKER_NODES_FILE=/etc/sub-maker/nodes.txt",
		"ExecStart=/usr/local/bin/sub-maker --serve",
	} {
		if !strings.Contains(unit, want) {
			t.Fatalf("rendered unit missing %q:\n%s", want, unit)
		}
	}
}

func TestRenderSingBoxConfigUsesEmbeddedBaseTemplate(t *testing.T) {
	t.Chdir(t.TempDir())
	etcSB := t.TempDir()
	cfg := &config.SetupConfig{
		EnabledProtocols: []string{"ss2022"},
		Ports:            map[string]int{"ss2022": 8388},
		Creds: map[string]map[string]string{
			"ss2022": {"pass": "test-password"},
		},
	}

	if err := renderSingBoxConfig(cfg, etcSB); err != nil {
		t.Fatalf("renderSingBoxConfig returned error: %v", err)
	}

	data, err := os.ReadFile(filepath.Join(etcSB, "config.json"))
	if err != nil {
		t.Fatalf("read config.json: %v", err)
	}
	var parsed struct {
		Inbounds []map[string]interface{} `json:"inbounds"`
	}
	if err := json.Unmarshal(data, &parsed); err != nil {
		t.Fatalf("generated sing-box config is not valid JSON: %v\n%s", err, data)
	}
	if len(parsed.Inbounds) != 1 || parsed.Inbounds[0]["type"] != "shadowsocks" {
		t.Fatalf("unexpected inbounds: %#v", parsed.Inbounds)
	}
}

func TestEnsureRuntimeCredentialsFillsAllEnabledProtocols(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.Creds["reality"] = map[string]string{
		"private_key": "private",
		"pbk":         "public",
	}

	if err := ensureRuntimeCredentials(cfg); err != nil {
		t.Fatalf("ensureRuntimeCredentials returned error: %v", err)
	}

	required := map[string][]string{
		"reality":   {"uuid", "short_id", "private_key", "pbk"},
		"hysteria2": {"pass"},
		"tuic":      {"uuid", "pass"},
		"anytls":    {"pass"},
		"ss2022":    {"pass"},
	}
	for proto, keys := range required {
		for _, key := range keys {
			if cfg.Creds[proto][key] == "" {
				t.Fatalf("missing generated credential %s.%s", proto, key)
			}
		}
	}
}
