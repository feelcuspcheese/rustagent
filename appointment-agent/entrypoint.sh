#!/bin/sh
if [ ! -s /root/configs/config.yaml ]; then
    cp /root/configs/default_config.yaml /root/configs/config.yaml
fi
exec /usr/local/bin/agent --web
