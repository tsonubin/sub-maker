package config

import "time"

// SetupConfig holds all user choices from the TUI wizard. Persisted to /etc/sub-maker/config.yaml .
type SetupConfig struct {
	ServerAddr string            `yaml:"server_addr"` // public IP or domain for links
	Domain     string            `yaml:"domain"`      // for certs + reality sni default
	SubToken   string            `yaml:"sub_token"`
	SubPort    int               `yaml:"sub_port"` // default 8964
	Email      string            `yaml:"email"`    // for acme

	EnabledProtocols []string          `yaml:"enabled_protocols"` // e.g. ["reality","hysteria2",...]
	Ports            map[string]int    `yaml:"ports"`
	Creds            map[string]map[string]string `yaml:"creds"` // per proto extra (uuid, pass, pbk, short_id, sni, remark)

	CertMode     string `yaml:"cert_mode"` // "acme-http", "acme-dns-cf", "self-signed"
	ACMETokenCF  string `yaml:"acme_cf_token,omitempty"`
	CertPath     string `yaml:"cert_path"`
	KeyPath      string `yaml:"key_path"`

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
		Creds:    make(map[string]map[string]string),
		CertMode: "acme-http",
	}
}
