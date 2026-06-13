package cli

import (
	"os"
	"path/filepath"

	"github.com/tsonubin/sub-maker/internal/config"
	"gopkg.in/yaml.v3"
)

func ConfigPath() string {
	base := "/etc"
	if os.Getenv("SUB_MAKER_DEMO") != "" {
		base = "/tmp/sub-maker-demo-etc"
	}
	return filepath.Join(base, "sub-maker", "config.yaml")
}

func NodesPath() string {
	base := "/etc"
	if os.Getenv("SUB_MAKER_DEMO") != "" {
		base = "/tmp/sub-maker-demo-etc"
	}
	return filepath.Join(base, "sub-maker", "nodes.txt")
}

func LoadConfig() (*config.SetupConfig, error) {
	data, err := os.ReadFile(ConfigPath())
	if err != nil {
		return nil, err
	}
	var cfg config.SetupConfig
	if err := yaml.Unmarshal(data, &cfg); err != nil {
		return nil, err
	}
	return &cfg, nil
}
