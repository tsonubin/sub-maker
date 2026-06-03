{
  "type": "vless",
  "tag": "vless-reality-in",
  "listen": "::",
  "listen_port": {{ .Port }},
  "users": [
    {
      "name": "user",
      "uuid": "{{ .UUID }}"
    }
  ],
  "tls": {
    "enabled": true,
    "server_name": "{{ .ServerName }}",
    "reality": {
      "enabled": true,
      "handshake": {
        "server": "{{ .ServerName }}",
        "server_port": 443
      },
      "private_key": "{{ .PrivateKey }}",
      "short_id": ["{{ .ShortID }}"]
    }
  },
  "transport": {
    "type": "tcp"
  }
}
