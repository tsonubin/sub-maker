{
  "type": "hysteria2",
  "tag": "hysteria2-in",
  "listen": "::",
  "listen_port": {{ .Port }},
  "users": [
    {
      "password": "{{ .Password }}"
    }
  ],
  "tls": {
    "enabled": true,
    "server_name": "{{ .ServerName }}",
    "certificate_path": "{{ .CertPath }}",
    "key_path": "{{ .KeyPath }}"
  }
}
