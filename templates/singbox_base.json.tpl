{
  "log": {
    "level": "info",
    "timestamp": true
  },
  "dns": {
    "strategy": "ipv4_only",
    "servers": [
      {
        "type": "https",
        "server": "1.1.1.1"
      }
    ]
  },
  "inbounds": [
    {{ range $i, $in := .Inbounds -}}
    {{ if $i }},{{ end }}
    {{ $in | printf "%s" }}
    {{- end }}
  ],
  "outbounds": [
    {
      "type": "direct",
      "tag": "direct"
    }
  ]
}
