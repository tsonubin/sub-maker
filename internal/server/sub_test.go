package server

import (
	"strings"
	"testing"
)

func TestNodesToClashProxiesUsesProtocolAccurateFields(t *testing.T) {
	nodes := []string{
		"vless://uuid@1.2.3.4:443?security=reality&flow=xtls-rprx-vision&fp=chrome&sni=www.apple.com&sid=0123abcd&pbk=pubkey#Reality",
		"hysteria2://hy-pass@1.2.3.4:8443/?sni=example.com#Hy2",
		"tuic://tuic-uuid:tuic-pass@1.2.3.4:9443/?sni=example.com&alpn=h3#TUIC",
		"anytls://any-pass@1.2.3.4:7443?security=tls&sni=example.com#AnyTLS",
		"ss://MjAyMi1ibGFrZTMtYWVzLTEyOC1nY206MDEyMzQ1Njc4OWFiY2RlZg@1.2.3.4:8388#SS2022",
	}

	yaml, names := nodesToClashProxiesWithNames(nodes)
	if len(names) != len(nodes) {
		t.Fatalf("expected %d names, got %d", len(nodes), len(names))
	}

	checks := []string{
		"type: vless",
		"flow: xtls-rprx-vision",
		"packet-encoding: xudp",
		"servername: \"www.apple.com\"",
		"type: hysteria2",
		"alpn: [h3]",
		"type: tuic",
		"udp-relay-mode: native",
		"congestion-controller: bbr",
		"type: anytls",
		"cipher: \"2022-blake3-aes-128-gcm\"",
		"password: \"0123456789abcdef\"",
		"udp: true",
	}
	for _, check := range checks {
		if !strings.Contains(yaml, check) {
			t.Fatalf("expected YAML to contain %q:\n%s", check, yaml)
		}
	}
	if strings.Contains(yaml, "type: trojan") {
		t.Fatalf("AnyTLS must not be mapped to Trojan:\n%s", yaml)
	}
}
