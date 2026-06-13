package cli

import (
	"bufio"
	"fmt"
	"os"
	"strings"

	"github.com/tsonubin/sub-maker/internal/diagnostics"
	"golang.org/x/term"
)

func PrintSubscriptionLinkWithPasscode(passcode string, readStdin bool) error {
	cfg, err := LoadConfig()
	if err != nil {
		return err
	}
	if cfg.LinkPasscodeSalt == "" || cfg.LinkPasscodeHash == "" {
		return fmt.Errorf("SSH link passcode is not configured; rerun `sudo sub-maker setup`")
	}
	if passcode == "" {
		passcode, err = readPasscode(readStdin)
		if err != nil {
			return err
		}
	}
	if !cfg.VerifyLinkPasscode(passcode) {
		return fmt.Errorf("invalid passcode")
	}
	fmt.Println(diagnostics.BuildLinks(cfg).Subscription)
	return nil
}

func readPasscode(readStdin bool) (string, error) {
	if readStdin {
		scanner := bufio.NewScanner(os.Stdin)
		if !scanner.Scan() {
			if err := scanner.Err(); err != nil {
				return "", err
			}
			return "", fmt.Errorf("missing passcode on stdin")
		}
		return strings.TrimSpace(scanner.Text()), nil
	}
	if term.IsTerminal(int(os.Stdin.Fd())) {
		fmt.Fprint(os.Stderr, "Passcode: ")
		value, err := term.ReadPassword(int(os.Stdin.Fd()))
		fmt.Fprintln(os.Stderr)
		if err != nil {
			return "", err
		}
		return strings.TrimSpace(string(value)), nil
	}
	return "", fmt.Errorf("passcode required; use --passcode-stdin for non-interactive SSH commands")
}
