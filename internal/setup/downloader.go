package setup

import (
	"archive/tar"
	"compress/gzip"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"time"

	"github.com/tsonubin/sub-maker/internal/config"
)

var downloadHTTPClient = &http.Client{Timeout: 2 * time.Minute}
var maxDownloadBytes int64 = 256 << 20

func downloadFile(url, dest string) error {
	resp, err := downloadHTTPClient.Get(url)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode != 200 {
		return fmt.Errorf("bad status: %s", resp.Status)
	}

	tmpDest := dest + ".tmp"
	cleanupTmp := true
	defer func() {
		if cleanupTmp {
			_ = os.Remove(tmpDest)
		}
	}()

	out, err := os.Create(tmpDest)
	if err != nil {
		return err
	}
	written, err := io.Copy(out, io.LimitReader(resp.Body, maxDownloadBytes+1))
	closeErr := out.Close()
	if err != nil {
		return err
	}
	if closeErr != nil {
		return closeErr
	}
	if written > maxDownloadBytes {
		return fmt.Errorf("download exceeds max size of %d bytes", maxDownloadBytes)
	}
	if err := os.Rename(tmpDest, dest); err != nil {
		return err
	}
	cleanupTmp = false
	return nil
}

func extractTarGz(src, dest string) error {
	f, err := os.Open(src)
	if err != nil {
		return err
	}
	defer f.Close()
	gz, err := gzip.NewReader(f)
	if err != nil {
		return err
	}
	defer gz.Close()
	tr := tar.NewReader(gz)
	for {
		h, err := tr.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			return err
		}
		target := filepath.Join(dest, h.Name)
		if !strings.HasPrefix(filepath.Clean(target), filepath.Clean(dest)+string(os.PathSeparator)) && filepath.Clean(target) != filepath.Clean(dest) {
			return fmt.Errorf("tar entry %q attempts path traversal", h.Name)
		}
		switch h.Typeflag {
		case tar.TypeDir:
			if err := os.MkdirAll(target, 0755); err != nil {
				return err
			}
		case tar.TypeReg:
			if err := os.MkdirAll(filepath.Dir(target), 0755); err != nil {
				return err
			}
			out, err := os.Create(target)
			if err != nil {
				return err
			}
			if _, err := io.Copy(out, tr); err != nil {
				out.Close()
				return err
			}
			if err := out.Close(); err != nil {
				return err
			}
			// make executable if looks like bin
			if strings.Contains(target, "sing-box") || strings.Contains(target, "subconverter") {
				if err := os.Chmod(target, 0755); err != nil {
					return err
				}
			}
		}
	}
	return nil
}

// DownloadSingBox downloads and installs sing-box binary to /usr/local/bin/sing-box
// In DEMO mode (SUB_MAKER_DEMO=1), installs to $HOME/.local/bin/sing-box (no sudo needed).
func DownloadSingBox(version string) error {
	if version == "" {
		version = latestGitHubReleaseTag("SagerNet/sing-box", "v1.12.0")
	}
	arch := runtime.GOARCH
	if arch == "amd64" {
		arch = "amd64"
	} else if arch == "arm64" {
		arch = "arm64"
	} else {
		arch = "amd64"
	}
	url := fmt.Sprintf("https://github.com/SagerNet/sing-box/releases/download/%s/sing-box-%s-linux-%s.tar.gz", version, strings.TrimPrefix(version, "v"), arch)
	tmp := "/tmp/sing-box.tar.gz"
	if err := downloadFile(url, tmp); err != nil {
		return fmt.Errorf("download sing-box: %w", err)
	}
	extractDir := "/tmp/sing-box-extract"
	if err := os.RemoveAll(extractDir); err != nil {
		return err
	}
	if err := os.MkdirAll(extractDir, 0755); err != nil {
		return err
	}
	if err := extractTarGz(tmp, extractDir); err != nil {
		return err
	}
	// find the binary inside (usually sing-box-{ver}-linux-{arch}/sing-box )
	bin := ""
	if err := filepath.Walk(extractDir, func(p string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		if !info.IsDir() && strings.HasSuffix(p, "sing-box") {
			bin = p
		}
		return nil
	}); err != nil {
		return err
	}
	if bin == "" {
		return fmt.Errorf("sing-box binary not found in archive")
	}
	installPath := "/usr/local/bin/sing-box"
	if config.DemoMode() {
		installDir := filepath.Join(homeDir(), ".local/bin")
		if err := os.MkdirAll(installDir, 0755); err != nil {
			return err
		}
		installPath = filepath.Join(installDir, "sing-box")
	}
	return os.Rename(bin, installPath)
}

func latestGitHubReleaseTag(repo, fallback string) string {
	req, err := http.NewRequest(http.MethodGet, "https://api.github.com/repos/"+repo+"/releases/latest", nil)
	if err != nil {
		return fallback
	}
	req.Header.Set("Accept", "application/vnd.github+json")
	client := &http.Client{Timeout: 5 * time.Second}
	resp, err := client.Do(req)
	if err != nil {
		return fallback
	}
	defer resp.Body.Close()
	if resp.StatusCode < http.StatusOK || resp.StatusCode >= http.StatusMultipleChoices {
		return fallback
	}
	var payload struct {
		TagName string `json:"tag_name"`
	}
	if err := json.NewDecoder(io.LimitReader(resp.Body, 1<<20)).Decode(&payload); err != nil {
		return fallback
	}
	if payload.TagName == "" {
		return fallback
	}
	return payload.TagName
}

// DownloadSubconverter downloads the asdlokj1qpi233 subconverter fork and places it in /opt/subconverter.
// The fork ships multi-arch builds (linux64, aarch64, armv7, linux32).
// In DEMO mode (SUB_MAKER_DEMO=1), installs to $HOME/.local/subconverter (no sudo needed).
func DownloadSubconverter(version string) error {
	if version == "" {
		version = latestGitHubReleaseTag("asdlokj1qpi233/subconverter", "v0.9.9")
	}
	asset := subconverterAssetName()
	url := fmt.Sprintf("https://github.com/asdlokj1qpi233/subconverter/releases/download/%s/%s", version, asset)
	tmp := "/tmp/subconverter.tar.gz"
	if err := downloadFile(url, tmp); err != nil {
		return fmt.Errorf("download subconverter: %w", err)
	}
	dest := subconverterDir()
	if err := os.RemoveAll(dest); err != nil {
		return err
	}
	if err := os.MkdirAll(dest, 0755); err != nil {
		return err
	}
	return extractTarGz(tmp, dest)
}

func subconverterAssetName() string {
	switch runtime.GOARCH {
	case "arm64":
		return "subconverter_aarch64.tar.gz"
	case "arm":
		return "subconverter_armv7.tar.gz"
	case "386":
		return "subconverter_linux32.tar.gz"
	default:
		return "subconverter_linux64.tar.gz"
	}
}
