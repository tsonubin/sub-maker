package config

import (
	"crypto/rand"
	"crypto/sha256"
	"crypto/subtle"
	"encoding/base64"
	"fmt"
	"time"
)

type SetupMode string

const (
	SetupModeProduction SetupMode = "production"
	SetupModeIPOnly     SetupMode = "ip-only"
)

type CertStrategy string

const (
	CertStrategyCertbotHTTP CertStrategy = "certbot-http"
	CertStrategyACMEHTTP    CertStrategy = "acme-http"
	CertStrategyACMEDNSCF   CertStrategy = "acme-dns-cf"
	CertStrategyExisting    CertStrategy = "existing"
	CertStrategySelfSigned  CertStrategy = "self-signed"
)

type DNSCheckResult struct {
	Domain        string   `yaml:"domain"`
	ExpectedIPv4  string   `yaml:"expected_ipv4,omitempty"`
	ExpectedIPv6  string   `yaml:"expected_ipv6,omitempty"`
	ResolvedA     []string `yaml:"resolved_a,omitempty"`
	ResolvedAAAA  []string `yaml:"resolved_aaaa,omitempty"`
	IPv4Matches   bool     `yaml:"ipv4_matches"`
	IPv6Matches   bool     `yaml:"ipv6_matches"`
	LastCheckedAt string   `yaml:"last_checked_at,omitempty"`
}

// SetupConfig holds all user choices from the TUI wizard. Persisted to /etc/sub-maker/config.yaml .
type SetupConfig struct {
	ServerAddr string          `yaml:"server_addr"` // public IP or domain for links
	Domain     string          `yaml:"domain"`      // for certs + reality sni default
	SubToken   string          `yaml:"sub_token"`
	SubPort    int             `yaml:"sub_port"` // default 8964
	Email      string          `yaml:"email"`    // for acme
	SetupMode  SetupMode       `yaml:"setup_mode"`
	DNSCheck   *DNSCheckResult `yaml:"dns_check,omitempty"`

	LinkPasscodeSalt  string `yaml:"link_passcode_salt,omitempty"`
	LinkPasscodeHash  string `yaml:"link_passcode_hash,omitempty"`
	LinkPasscodePlain string `yaml:"-"`

	EnabledProtocols []string                     `yaml:"enabled_protocols"` // e.g. ["reality","hysteria2",...]
	Ports            map[string]int               `yaml:"ports"`
	Creds            map[string]map[string]string `yaml:"creds"` // per proto extra (uuid, pass, pbk, short_id, sni, remark)

	CertMode    CertStrategy `yaml:"cert_mode"`
	ACMETokenCF string       `yaml:"acme_cf_token,omitempty"`
	CertPath    string       `yaml:"cert_path"`
	KeyPath     string       `yaml:"key_path"`

	CreatedAt time.Time `yaml:"created_at"`
	UpdatedAt time.Time `yaml:"updated_at"`
}

func DefaultConfig() *SetupConfig {
	return &SetupConfig{
		SubPort:          8964,
		EnabledProtocols: []string{"reality", "hysteria2", "tuic", "anytls", "ss2022"},
		Ports: map[string]int{
			"reality":   443,
			"hysteria2": 8443,
			"tuic":      9443,
			"anytls":    7443,
			"ss2022":    8388,
		},
		Creds:     make(map[string]map[string]string),
		CertMode:  CertStrategyCertbotHTTP,
		SetupMode: SetupModeProduction,
	}
}

func GenerateLinkPasscode() string {
	const alphabet = "23456789abcdefghjkmnpqrstuvwxyz"
	b := make([]byte, 10)
	if _, err := rand.Read(b); err != nil {
		return "change-me"
	}
	out := make([]byte, len(b))
	for i, value := range b {
		out[i] = alphabet[int(value)%len(alphabet)]
	}
	return string(out)
}

func (cfg *SetupConfig) SetLinkPasscode(passcode string) error {
	if passcode == "" {
		return fmt.Errorf("passcode is required")
	}
	salt := make([]byte, 16)
	if _, err := rand.Read(salt); err != nil {
		return err
	}
	saltText := base64.RawURLEncoding.EncodeToString(salt)
	cfg.LinkPasscodeSalt = saltText
	cfg.LinkPasscodeHash = hashLinkPasscode(saltText, passcode)
	cfg.LinkPasscodePlain = passcode
	return nil
}

func (cfg *SetupConfig) VerifyLinkPasscode(passcode string) bool {
	if cfg == nil || cfg.LinkPasscodeSalt == "" || cfg.LinkPasscodeHash == "" || passcode == "" {
		return false
	}
	expected := hashLinkPasscode(cfg.LinkPasscodeSalt, passcode)
	return subtle.ConstantTimeCompare([]byte(expected), []byte(cfg.LinkPasscodeHash)) == 1
}

func hashLinkPasscode(salt, passcode string) string {
	sum := sha256.Sum256([]byte(salt + "\x00" + passcode))
	return base64.RawURLEncoding.EncodeToString(sum[:])
}
