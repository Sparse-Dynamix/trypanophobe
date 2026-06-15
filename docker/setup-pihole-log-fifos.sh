#!/bin/sh
# Pi-hole opens log paths before reading pihole.toml; FIFOs under /tmp stream to stdout.
for name in ftl dnsmasq web; do
  pipe="/tmp/pihole-${name}.pipe"
  rm -f "$pipe"
  mkfifo -m 666 "$pipe"
  ( while cat "$pipe"; do :; done ) >&1 &
done
mkdir -p /var/log/pihole
ln -sf /tmp/pihole-ftl.pipe /var/log/pihole/FTL.log
ln -sf /tmp/pihole-dnsmasq.pipe /var/log/pihole/pihole.log
ln -sf /tmp/pihole-web.pipe /var/log/pihole/webserver.log

export FTLCONF_files_log_ftl="/tmp/pihole-ftl.pipe"
export FTLCONF_files_log_dnsmasq="/tmp/pihole-dnsmasq.pipe"
export FTLCONF_files_log_webserver="/tmp/pihole-web.pipe"
export FTLCONF_files_pid="/tmp/pihole-FTL.pid"
export FTLCONF_files_database="/tmp/pihole-FTL.db"
