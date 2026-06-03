package generator

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestGenerateAll_Top5Formats(t *testing.T) {
	ports := map[string]int{
		"reality":   443,
		"hysteria2": 8443,
		"tuic":      9443,
		"anytls":    7443,
		"ss2022":    8388,
	}
	creds := map[string]map[string]string{
		"reality": {"sni": "www.apple.com", "pbk": "testpbk1234567890abcdef", "short_id": "01234567"},
		"hysteria2": {"sni": "example.com"},
		"tuic":      {"sni": "example.com"},
		"anytls":    {"sni": "example.com"},
		"ss2022":    {},
	}

	nodes := GenerateAll("1.2.3.4", "example.com", ports, creds)
	if len(nodes) != 5 {
		t.Fatalf("expected 5 nodes, got %d", len(nodes))
	}

	for _, n := range nodes {
		switch n.Protocol {
		case "vless-reality":
			if !strings.Contains(n.URI, "security=reality") || !strings.Contains(n.URI, "flow=xtls-rprx-vision") || !strings.Contains(n.URI, "sni=www.apple.com") {
				t.Errorf("bad reality uri: %s", n.URI)
			}
		case "hysteria2":
			if !strings.HasPrefix(n.URI, "hysteria2://") || !strings.Contains(n.URI, "sni=example.com") {
				t.Errorf("bad hy2 uri: %s", n.URI)
			}
		case "tuic":
			if !strings.HasPrefix(n.URI, "tuic://") || !strings.Contains(n.URI, "sni=example.com") {
				t.Errorf("bad tuic uri: %s", n.URI)
			}
		case "anytls":
			if !strings.HasPrefix(n.URI, "anytls://") || !strings.Contains(n.URI, "security=tls") {
				t.Errorf("bad anytls uri: %s", n.URI)
			}
		case "ss2022":
			if !strings.HasPrefix(n.URI, "ss://") {
				t.Errorf("bad ss2022 uri: %s", n.URI)
			}
			// URI uses base64(method:pass)@host , check contains 2022-blake after decode not necessary here.
		}
	}
}

func TestWriteNodesFile(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "nodes.txt")
	nodes := []Node{
		{URI: "hysteria2://p@1.2.3.4:8443/?sni=ex.com#Hy2"},
		{URI: "vless://u@1.2.3.4:443?security=reality#R"},
	}
	if err := WriteNodesFile(path, nodes); err != nil {
		t.Fatal(err)
	}
	data, _ := os.ReadFile(path)
	content := string(data)
	if !strings.Contains(content, "hysteria2://") || !strings.Contains(content, "vless://") {
		t.Errorf("nodes file content wrong: %s", content)
	}
	lines, _ := LoadNodesFromFile(path)
	if len(lines) != 2 {
		t.Errorf("expected 2 lines, got %d", len(lines))
	}
}
