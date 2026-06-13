package cli

import (
	"fmt"

	"github.com/tsonubin/sub-maker/internal/diagnostics"
)

var managedServices = []string{"sing-box", "sub-maker-sub"}

func ServiceCommand(action string) error {
	args := []string{action}
	args = append(args, managedServices...)
	switch action {
	case "status":
		args = append(args, "--no-pager")
		out, err := diagnostics.SystemctlOutput(args...)
		fmt.Print(out)
		return err
	case "start", "stop", "restart":
		return diagnostics.RunSystemctl(args...)
	default:
		return fmt.Errorf("unsupported service command: %s", action)
	}
}
