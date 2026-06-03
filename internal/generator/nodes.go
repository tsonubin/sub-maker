package generator

import (
	"crypto/rand"
	"encoding/base64"
	"encoding/hex"
	"fmt"
	"net/url"
	"os"
	"strings"

	"github.com/google/uuid"
)

// Node represents one generated inbound share link (in URI format subconverter understands).
type Node struct {
	Protocol string
	URI      string
	Remark   string
}

// randomShortID returns 8 hex chars suitable for Reality sid.
func randomShortID() string {
	b := make([]byte, 4)
	_, _ = rand.Read(b)
	return hex.EncodeToString(b)
}

// randomPass returns a base64 password suitable for many protos (Hy2, AnyTLS, SS).
func randomPass() string {
	b := make([]byte, 16)
	_, _ = rand.Read(b)
	return base64.RawStdEncoding.EncodeToString(b)
}

// GenerateVLESSReality produces a VLESS + Reality-Vision link.
// Uses flow=xtls-rprx-vision for Vision. serverName is the "borrowed" real site (e.g. www.apple.com).
func GenerateVLESSReality(serverAddr string, port int, domainForSNI string, uuidStr, publicKey, shortID string, remark string) string {
	if uuidStr == "" {
		uuidStr = uuid.New().String()
	}
	if shortID == "" {
		shortID = randomShortID()
	}
	q := url.Values{}
	q.Set("type", "tcp")
	q.Set("security", "reality")
	q.Set("flow", "xtls-rprx-vision")
	q.Set("pbk", publicKey)
	q.Set("fp", "chrome")
	q.Set("sni", domainForSNI)
	q.Set("sid", shortID)
	q.Set("packetEncoding", "xudp")
	return fmt.Sprintf("vless://%s@%s:%d?%s#%s", uuidStr, serverAddr, port, q.Encode(), url.PathEscape(remark))
}

// GenerateHysteria2 produces a Hysteria2 link. For real certs, insecure=0 + sni=domain.
func GenerateHysteria2(serverAddr string, port int, password, sni string, remark string, insecure bool) string {
	if password == "" {
		password = randomPass()
	}
	q := url.Values{}
	q.Set("sni", sni)
	if insecure {
		q.Set("insecure", "1")
	} else {
		q.Set("insecure", "0")
	}
	// Common: up/down mps can be in client, not required in URI for sub.
	return fmt.Sprintf("hysteria2://%s@%s:%d/?%s#%s", password, serverAddr, port, q.Encode(), url.PathEscape(remark))
}

// GenerateTUIC produces a TUIC v5 link.
func GenerateTUIC(serverAddr string, port int, uuidStr, password, sni, alpn string, remark string) string {
	if uuidStr == "" {
		uuidStr = uuid.New().String()
	}
	if password == "" {
		password = randomPass()
	}
	if alpn == "" {
		alpn = "h3"
	}
	q := url.Values{}
	q.Set("sni", sni)
	q.Set("alpn", alpn)
	q.Set("congestion_control", "bbr")
	q.Set("udp_relay_mode", "native")
	q.Set("allow_insecure", "0")
	return fmt.Sprintf("tuic://%s:%s@%s:%d/?%s#%s", uuidStr, password, serverAddr, port, q.Encode(), url.PathEscape(remark))
}

// GenerateAnyTLS produces an AnyTLS link (sing-box 1.12+). Uses tls + optional reality? For now basic tls camo.
func GenerateAnyTLS(serverAddr string, port int, password, sni string, remark string) string {
	if password == "" {
		password = randomPass()
	}
	q := url.Values{}
	q.Set("security", "tls")
	q.Set("sni", sni)
	q.Set("fp", "chrome")
	// padding_scheme etc usually server side; client URI is simple.
	return fmt.Sprintf("anytls://%s@%s:%d?%s#%s", password, serverAddr, port, q.Encode(), url.PathEscape(remark))
}

// GenerateSS2022 produces a Shadowsocks 2022 (AEAD) link. Method 2022-blake3-aes-128-gcm or 256.
func GenerateSS2022(serverAddr string, port int, password, method string, remark string) string {
	if password == "" {
		password = randomPass()
	}
	if method == "" {
		method = "2022-blake3-aes-128-gcm"
	}
	userinfo := fmt.Sprintf("%s:%s", method, password)
	encoded := base64.RawURLEncoding.EncodeToString([]byte(userinfo))
	return fmt.Sprintf("ss://%s@%s:%d#%s", encoded, serverAddr, port, url.PathEscape(remark))
}

// GenerateAll builds the default top-5 nodes. creds and ports come from TUI collected data.
// ports keys: "reality", "hysteria2", "tuic", "anytls", "ss2022"
// creds[proto]["uuid"|"pass"|"pbk"|"short_id"|"sni" ...]
func GenerateAll(serverAddr, baseDomain string, ports map[string]int, creds map[string]map[string]string) []Node {
	if serverAddr == "" {
		serverAddr = "127.0.0.1"
	}
	nodes := []Node{}

	// 1. VLESS Reality Vision (stealth)
	realityCred := creds["reality"]
	if realityCred == nil {
		realityCred = map[string]string{}
	}
	realityPort := ports["reality"]
	if realityPort == 0 {
		realityPort = 443
	}
	sni := realityCred["sni"]
	if sni == "" {
		sni = baseDomain
		if sni == "" {
			sni = "www.microsoft.com"
		}
	}
	pbk := realityCred["pbk"]
	if pbk == "" {
		pbk = "stub-public-key-replace-after-reality-gen" // real one generated at setup from priv
	}
	sid := realityCred["short_id"]
	uuidR := realityCred["uuid"]
	remarkR := "VLESS-Reality"
	if r, ok := realityCred["remark"]; ok && r != "" {
		remarkR = r
	}
	nodes = append(nodes, Node{
		Protocol: "vless-reality",
		Remark:   remarkR,
		URI:      GenerateVLESSReality(serverAddr, realityPort, sni, uuidR, pbk, sid, remarkR),
	})

	// 2. Hysteria2
	hyCred := creds["hysteria2"]
	if hyCred == nil {
		hyCred = map[string]string{}
	}
	hyPort := ports["hysteria2"]
	if hyPort == 0 {
		hyPort = 8443
	}
	hyPass := hyCred["pass"]
	hySNI := hyCred["sni"]
	if hySNI == "" {
		hySNI = baseDomain
	}
	hyRemark := "Hysteria2"
	if r, ok := hyCred["remark"]; ok && r != "" {
		hyRemark = r
	}
	nodes = append(nodes, Node{
		Protocol: "hysteria2",
		Remark:   hyRemark,
		URI:      GenerateHysteria2(serverAddr, hyPort, hyPass, hySNI, hyRemark, false),
	})

	// 3. TUIC v5
	tuicCred := creds["tuic"]
	if tuicCred == nil {
		tuicCred = map[string]string{}
	}
	tuicPort := ports["tuic"]
	if tuicPort == 0 {
		tuicPort = 9443
	}
	tuicUUID := tuicCred["uuid"]
	tuicPass := tuicCred["pass"]
	tuicSNI := tuicCred["sni"]
	if tuicSNI == "" {
		tuicSNI = baseDomain
	}
	tuicRemark := "TUICv5"
	if r, ok := tuicCred["remark"]; ok && r != "" {
		tuicRemark = r
	}
	nodes = append(nodes, Node{
		Protocol: "tuic",
		Remark:   tuicRemark,
		URI:      GenerateTUIC(serverAddr, tuicPort, tuicUUID, tuicPass, tuicSNI, "h3", tuicRemark),
	})

	// 4. AnyTLS
	anyCred := creds["anytls"]
	if anyCred == nil {
		anyCred = map[string]string{}
	}
	anyPort := ports["anytls"]
	if anyPort == 0 {
		anyPort = 7443
	}
	anyPass := anyCred["pass"]
	anySNI := anyCred["sni"]
	if anySNI == "" {
		anySNI = baseDomain
	}
	anyRemark := "AnyTLS"
	if r, ok := anyCred["remark"]; ok && r != "" {
		anyRemark = r
	}
	nodes = append(nodes, Node{
		Protocol: "anytls",
		Remark:   anyRemark,
		URI:      GenerateAnyTLS(serverAddr, anyPort, anyPass, anySNI, anyRemark),
	})

	// 5. Shadowsocks 2022
	ssCred := creds["ss2022"]
	if ssCred == nil {
		ssCred = map[string]string{}
	}
	ssPort := ports["ss2022"]
	if ssPort == 0 {
		ssPort = 8388
	}
	ssPass := ssCred["pass"]
	ssMethod := ssCred["method"]
	if ssMethod == "" {
		ssMethod = "2022-blake3-aes-128-gcm"
	}
	ssRemark := "SS2022"
	if r, ok := ssCred["remark"]; ok && r != "" {
		ssRemark = r
	}
	nodes = append(nodes, Node{
		Protocol: "ss2022",
		Remark:   ssRemark,
		URI:      GenerateSS2022(serverAddr, ssPort, ssPass, ssMethod, ssRemark),
	})

	return nodes
}

// WriteNodesFile writes one URI per line (suitable for subconverter file:// source).
func WriteNodesFile(path string, nodes []Node) error {
	var b strings.Builder
	for _, n := range nodes {
		if n.URI != "" {
			b.WriteString(n.URI)
			b.WriteString("\n")
		}
	}
	return os.WriteFile(path, []byte(b.String()), 0640)
}

// LoadNodesFromFile is a helper (for tests or server).
func LoadNodesFromFile(path string) ([]string, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	lines := strings.Split(strings.TrimSpace(string(data)), "\n")
	return lines, nil
}

