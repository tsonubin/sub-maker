{
  "type": "anytls",
  "tag": "anytls-in",
  "listen": "::",
  "listen_port": {{ .Port }},
  "users": [
    {
      "name": "user",
      "password": "{{ .Password }}"
    }
  ],
  "padding_scheme": [],
  "tls": {
    "enabled": true,
    "server_name": "{{ .ServerName }}",
    "certificate_path": "{{ .CertPath }}",
    "key_path": "{{ .KeyPath }}",
    "utls": {
      "enabled": true,
      "fingerprint": "chrome"
    }
  }
}
