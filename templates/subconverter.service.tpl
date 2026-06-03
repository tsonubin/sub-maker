[Unit]
Description=subconverter service (for sub-maker)
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/opt/subconverter
ExecStart=/opt/subconverter/subconverter
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
