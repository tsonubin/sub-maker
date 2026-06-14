package main

import (
	"flag"
	"fmt"
	"log/slog"
	"os"
	"time"

	"github.com/tsonubin/sub-maker/internal/cli"
	"github.com/tsonubin/sub-maker/internal/config"
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
		fmt.Println("  sudo sub-maker config")
		fmt.Println("  sudo sub-maker doctor")
		fmt.Println("  sudo sub-maker link")
		fmt.Println("  sudo sub-maker links")
		fmt.Println("  sudo sub-maker restart")
	}

	flag.Parse()

	if *versionCmd {
		fmt.Printf("sub-maker %s (see plan.md for design)\n", version)
		return
	}

	slog.SetDefault(slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelInfo})))

	command := flag.Arg(0)
	switch {
	case *setupCmd || command == "setup":
		runSetup()
	case command == "config" || command == "configure":
		runConfig()
	case *serveCmd || command == "serve":
		if err := server.Run(); err != nil {
			slog.Error("server failed", "err", err)
			os.Exit(1)
		}
	case *nodesCmd || command == "nodes":
		data, err := os.ReadFile(cli.NodesPath())
		if err != nil {
			slog.Error("read nodes failed; run setup first", "path", cli.NodesPath(), "err", err)
			os.Exit(1)
		}
		fmt.Print(string(data))
	case *updateCmd || command == "update":
		fmt.Println("Update logic not yet implemented (task 9).")
	case command == "links":
		if err := cli.PrintLinks(); err != nil {
			slog.Error("links failed", "err", err)
			os.Exit(1)
		}
	case command == "link":
		linkFlags := flag.NewFlagSet("link", flag.ExitOnError)
		passcode := linkFlags.String("passcode", "", "Passcode for SSH link retrieval")
		passcodeStdin := linkFlags.Bool("passcode-stdin", false, "Read passcode from stdin")
		if err := linkFlags.Parse(flag.Args()[1:]); err != nil {
			slog.Error("link flags failed", "err", err)
			os.Exit(1)
		}
		if err := cli.PrintSubscriptionLinkWithPasscode(*passcode, *passcodeStdin); err != nil {
			slog.Error("link failed", "err", err)
			os.Exit(1)
		}
	case command == "doctor":
		if err := cli.Doctor(); err != nil {
			os.Exit(1)
		}
	case command == "status" || command == "restart" || command == "start" || command == "stop":
		if err := cli.ServiceCommand(command); err != nil {
			slog.Error(command+" failed", "err", err)
			os.Exit(1)
		}
	default:
		// No subcommand provided — show usage (includes disclaimer)
		flag.Usage()
	}

	_ = time.Now() // keep import if needed
}

func runSetup() {
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
	if err := setup.Apply(cfg); err != nil {
		slog.Error("apply failed", "err", err)
		os.Exit(1)
	}
}

func runConfig() {
	cfg, err := cli.LoadConfig()
	if err != nil {
		slog.Error("load config failed; run setup first", "path", cli.ConfigPath(), "err", err)
		os.Exit(1)
	}
	cfg, err = tui.RunConfigWizard(cfg)
	if err != nil {
		slog.Error("config wizard failed", "err", err)
		os.Exit(1)
	}
	if err := setup.Apply(cfg); err != nil {
		slog.Error("apply failed", "err", err)
		os.Exit(1)
	}
}
