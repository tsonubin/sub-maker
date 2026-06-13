package diagnostics

import (
	"fmt"

	"github.com/tsonubin/sub-maker/internal/config"
)

type Links struct {
	Subscription string
	Raw          string
	IPFallback   string
}

func BuildLinks(cfg *config.SetupConfig) Links {
	if cfg == nil {
		return Links{}
	}
	host := cfg.Domain
	if host == "" {
		host = cfg.ServerAddr
	}
	return Links{
		Subscription: fmt.Sprintf("http://%s:%d/sub?token=%s", host, cfg.SubPort, cfg.SubToken),
		Raw:          fmt.Sprintf("http://%s:%d/raw?token=%s", host, cfg.SubPort, cfg.SubToken),
		IPFallback:   fmt.Sprintf("http://%s:%d/sub?token=%s", cfg.ServerAddr, cfg.SubPort, cfg.SubToken),
	}
}
