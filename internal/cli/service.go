package cli

import (
	"fmt"

	"github.com/tsonubin/sub-maker/internal/diagnostics"
	"github.com/tsonubin/sub-maker/internal/setup"
)

var managedServices = []string{"sing-box", "subconverter", "sub-maker-sub"}

func ServiceCommand(action string) error {
	services := managedServices
	if action == "start" || action == "stop" || action == "restart" {
		services = installedManagedServices()
	}
	args := []string{action}
	args = append(args, services...)
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

func installedManagedServices() []string {
	out := make([]string, 0, len(managedServices))
	for _, service := range managedServices {
		if service == "subconverter" && !setup.SubconverterInstalled() {
			continue
		}
		out = append(out, service)
	}
	return out
}
