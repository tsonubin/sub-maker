package setup

import "testing"

func TestParseRealityKeypairOutputAcceptsSingBoxLabels(t *testing.T) {
	output := "PrivateKey: kLdPxlfZRBqpLh0Pf4C3JvVNvtHdCMCGnAjx2vA3KXU\nPublicKey: ghEQYzsLid4rCIyA694d399DIkYPuohND5NoqILIzSs\n"

	privateKey, publicKey, err := parseRealityKeypairOutput(output)
	if err != nil {
		t.Fatalf("parseRealityKeypairOutput returned error: %v", err)
	}
	if privateKey != "kLdPxlfZRBqpLh0Pf4C3JvVNvtHdCMCGnAjx2vA3KXU" {
		t.Fatalf("unexpected private key: %s", privateKey)
	}
	if publicKey != "ghEQYzsLid4rCIyA694d399DIkYPuohND5NoqILIzSs" {
		t.Fatalf("unexpected public key: %s", publicKey)
	}
}

func TestParseRealityKeypairOutputAcceptsSeparatedLabels(t *testing.T) {
	output := "Private key: private_value\nPublic key: public_value\n"

	privateKey, publicKey, err := parseRealityKeypairOutput(output)
	if err != nil {
		t.Fatalf("parseRealityKeypairOutput returned error: %v", err)
	}
	if privateKey != "private_value" || publicKey != "public_value" {
		t.Fatalf("unexpected keys: private=%s public=%s", privateKey, publicKey)
	}
}
