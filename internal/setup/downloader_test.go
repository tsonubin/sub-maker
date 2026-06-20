package setup

import (
	"archive/tar"
	"compress/gzip"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"
)

func TestDownloadFileWritesResponseBody(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_, _ = w.Write([]byte("archive"))
	}))
	defer server.Close()

	dest := filepath.Join(t.TempDir(), "download.tar.gz")
	if err := downloadFile(server.URL, dest); err != nil {
		t.Fatalf("downloadFile returned error: %v", err)
	}

	data, err := os.ReadFile(dest)
	if err != nil {
		t.Fatalf("read downloaded file: %v", err)
	}
	if string(data) != "archive" {
		t.Fatalf("unexpected downloaded content: %q", string(data))
	}
}

func TestDownloadFileRejectsOversizedResponse(t *testing.T) {
	originalLimit := maxDownloadBytes
	maxDownloadBytes = 4
	t.Cleanup(func() { maxDownloadBytes = originalLimit })

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_, _ = w.Write([]byte("too-large"))
	}))
	defer server.Close()

	dest := filepath.Join(t.TempDir(), "download.tar.gz")
	if err := downloadFile(server.URL, dest); err == nil {
		t.Fatal("expected oversized download error")
	}
	if _, err := os.Stat(dest); err == nil {
		t.Fatal("oversized response should not be promoted to destination file")
	}
}

func TestExtractTarGzRejectsPathTraversal(t *testing.T) {
	archive := createTarGz(t, []tarEntry{
		{name: "../escape.txt", body: "should not escape"},
	})
	dest := t.TempDir()
	if err := extractTarGz(archive, dest); err == nil {
		t.Fatal("expected path traversal error")
	}
	escaped := filepath.Join(filepath.Dir(dest), "escape.txt")
	if _, err := os.Stat(escaped); err == nil {
		t.Fatal("escaped file was written outside dest")
	}
}

func TestExtractTarGzExtractsValidEntries(t *testing.T) {
	archive := createTarGz(t, []tarEntry{
		{name: "bin/sing-box", body: "binary-content"},
		{name: "README.md", body: "readme"},
	})
	dest := t.TempDir()
	if err := extractTarGz(archive, dest); err != nil {
		t.Fatalf("extractTarGz returned error: %v", err)
	}
	data, err := os.ReadFile(filepath.Join(dest, "bin/sing-box"))
	if err != nil {
		t.Fatalf("expected bin/sing-box to exist: %v", err)
	}
	if string(data) != "binary-content" {
		t.Fatalf("unexpected content: %s", string(data))
	}
}

type tarEntry struct {
	name string
	body string
}

func createTarGz(t *testing.T, entries []tarEntry) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "test.tar.gz")
	f, err := os.Create(path)
	if err != nil {
		t.Fatalf("create tar.gz: %v", err)
	}
	defer f.Close()

	gw := gzip.NewWriter(f)
	tw := tar.NewWriter(gw)
	for _, e := range entries {
		hdr := &tar.Header{
			Name:     e.name,
			Mode:     0644,
			Size:     int64(len(e.body)),
			Typeflag: tar.TypeReg,
		}
		if err := tw.WriteHeader(hdr); err != nil {
			t.Fatalf("write header: %v", err)
		}
		if _, err := tw.Write([]byte(e.body)); err != nil {
			t.Fatalf("write body: %v", err)
		}
	}
	if err := tw.Close(); err != nil {
		t.Fatalf("close tar: %v", err)
	}
	if err := gw.Close(); err != nil {
		t.Fatalf("close gzip: %v", err)
	}
	return path
}
