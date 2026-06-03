{
  "type": "tuic",
  "tag": "tuic-in",
  "listen": "::",
  "listen_port": {{ .Port }},
  "users": [
    {
      "uuid": "{{ .UUID }}",
      "password": "{{ .Password }}"
    }
  ],
  "tls": {
    "enabled": true,
    "server_name": "{{ .ServerName }}",
    "certificate_path": "{{ .CertPath }}",
    "key_path": "{{ .KeyPath }}"
  },
  "congestion_control": "bbr",
  "auth_timeout": "3s",
  "udp_relay": {
    "enabled": true,
    "timeout": "60s"
  }
}
