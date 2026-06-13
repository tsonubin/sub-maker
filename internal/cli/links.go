package cli

import (
	"fmt"

	"github.com/tsonubin/sub-maker/internal/diagnostics"
)

func PrintLinks() error {
	cfg, err := LoadConfig()
	if err != nil {
		return err
	}
	links := diagnostics.BuildLinks(cfg)
	fmt.Printf("Subscription:\n  %s\n", links.Subscription)
	fmt.Printf("Raw nodes:\n  %s\n", links.Raw)
	if cfg.Domain != "" && cfg.ServerAddr != "" {
		fmt.Printf("IP fallback:\n  %s\n", links.IPFallback)
	}
	return nil
}
