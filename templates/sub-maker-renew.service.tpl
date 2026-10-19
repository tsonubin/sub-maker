[Unit]
Description=sub-maker certificate refresh
After=network.target

[Service]
Type=oneshot
ExecStart=/usr/local/bin/sub-maker --renew
