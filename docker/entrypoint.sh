#!/bin/sh
set -e
set -a
. /etc/trypanophobe/env.defaults
. /etc/trypanophobe/build.env
set +a
. /usr/local/bin/setup-pihole-log-fifos.sh
exec /usr/bin/supervisord -c /etc/trypanophobe/supervisord.conf
