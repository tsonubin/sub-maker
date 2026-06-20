[Unit]
Description=sub-maker subscription service
After=network.target subconverter.service
Wants=subconverter.service

[Service]
Type=simple
User=root
Environment=SUB_MAKER_TOKEN={{.Token}}
Environment=SUB_MAKER_PORT={{.Port}}
Environment=SUB_MAKER_NODES_FILE={{.NodesPath}}
ExecStart=/usr/local/bin/sub-maker --serve
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
