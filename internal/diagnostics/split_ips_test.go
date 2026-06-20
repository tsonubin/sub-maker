package diagnostics

import "testing"

func TestSplitExpectedIPsIPv4(t *testing.T) {
	v4, v6 := SplitExpectedIPs("43.245.198.178")
	if v4 != "43.245.198.178" || v6 != "" {
		t.Fatalf("expected (43.245.198.178, \"\"), got (%s, %s)", v4, v6)
	}
}

func TestSplitExpectedIPsIPv6(t *testing.T) {
	v4, v6 := SplitExpectedIPs("2001:db8::1")
	if v4 != "" || v6 != "2001:db8::1" {
		t.Fatalf("expected (\"\", 2001:db8::1), got (%s, %s)", v4, v6)
	}
}

func TestSplitExpectedIPsInvalid(t *testing.T) {
	v4, v6 := SplitExpectedIPs("not-an-ip")
	if v4 != "" || v6 != "" {
		t.Fatalf("expected (\"\", \"\"), got (%s, %s)", v4, v6)
	}
}

func TestSplitExpectedIPsEmpty(t *testing.T) {
	v4, v6 := SplitExpectedIPs("")
	if v4 != "" || v6 != "" {
		t.Fatalf("expected (\"\", \"\"), got (%s, %s)", v4, v6)
	}
}
