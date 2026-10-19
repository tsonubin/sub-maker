[Unit]
Description=Weekly sub-maker certificate refresh

[Timer]
OnCalendar=weekly
RandomizedDelaySec=1h
Persistent=true

[Install]
WantedBy=timers.target
