package main

import (
	"flag"
	"fmt"
	"log/slog"
	"os"
	"time"

	"github.com/tsonubin/sub-maker/internal/config"
	"github.com/tsonubin/sub-maker/internal/generator"
	"github.com/tsonubin/sub-maker/internal/server"
	"github.com/tsonubin/sub-maker/internal/setup"
	"github.com/tsonubin/sub-maker/internal/tui"
)

var version = "dev"


func main() {
	setupCmd := flag.Bool("setup", false, "Run interactive TUI setup wizard (collects info, generates nodes/certs stubs)")
	serveCmd := flag.Bool("serve", false, "Run the subscription server on :8964 (for systemd)")
	nodesCmd := flag.Bool("nodes", false, "Print current generated node URIs")
	updateCmd := flag.Bool("update", false, "Update sing-box and subconverter binaries")
	versionCmd := flag.Bool("version", false, "Print version")

	// Custom usage so that -h / --help (and bare invocation) show our disclaimer
	flag.Usage = func() {
		fmt.Println("sub-maker: TUI for top-5 sing-box proxy + Clash sub with subconverter rulesets")
		fmt.Println("See LICENSE and DISCLAIMER.md — very strict terms (forking/use requires written permission).")
		fmt.Println("Usage:")
		flag.PrintDefaults()
		fmt.Println("\nExamples:")
		fmt.Println("  sudo sub-maker --setup")
		fmt.Println("  SUB_MAKER_DEMO=1 sub-maker --setup")
		fmt.Println("  sub-maker --serve")
		fmt.Println("  sub-maker --nodes")
	}

	flag.Parse()

	if *versionCmd {
		fmt.Printf("sub-maker %s (see plan.md for design)\n", version)
		return
	}

	slog.SetDefault(slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelInfo})))

	switch {
	case *setupCmd:
		var cfg *config.SetupConfig
		if os.Getenv("SUB_MAKER_DEMO") != "" {
			// Non-interactive demo path (for CI / this env without tty)
			cfg = config.DefaultConfig()
			cfg.ServerAddr = "demo.server.example"
			cfg.Domain = "example.com"
			cfg.SubToken = "demo-token-123"
			slog.Info("DEMO mode: using defaults (no interactive TUI)")
		} else {
			var err error
			cfg, err = tui.RunWizard()
			if err != nil {
				slog.Error("wizard failed", "err", err)
				os.Exit(1)
			}
		}
		// Real apply (currently demo that writes to /tmp + prints instructions)
		if err := setup.Apply(cfg); err != nil {
			slog.Error("apply failed", "err", err)
			os.Exit(1)
		}

	case *serveCmd:
		if err := server.Run(); err != nil {
			slog.Error("server failed", "err", err)
			os.Exit(1)
		}
	case *nodesCmd:
		// In real: load /etc/sub-maker/nodes.txt or config
		fmt.Println("Demo nodes (run --setup first for real):")
		nodes := generator.GenerateAll("demo.server", "example.com", config.DefaultConfig().Ports, nil)
		for _, n := range nodes {
			fmt.Println(n.URI)
		}
	case *updateCmd:
		fmt.Println("Update logic not yet implemented (task 9).")
	default:
		// No subcommand provided — show usage (includes disclaimer)
		flag.Usage()
	}

	_ = time.Now() // keep import if needed
}
