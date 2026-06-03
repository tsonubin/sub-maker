{
  "log": {
    "level": "info",
    "timestamp": true
  },
  "dns": {
    "servers": [
      {
        "address": "https://1.1.1.1/dns-query",
        "strategy": "ipv4_only"
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
