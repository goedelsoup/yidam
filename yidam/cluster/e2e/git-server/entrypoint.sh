#!/bin/sh
# Host key and the two client keys come from a mounted secret, so `run.sh` writes the
# known_hosts line the pods check against. sshd's StrictModes refuses a secret mount's
# permissions, so each is copied into place first.
set -eu
install -m 600 /etc/git-server/ssh_host_ed25519_key /etc/ssh/ssh_host_ed25519_key
mkdir -p /home/git/.ssh
{
  printf 'command="/usr/local/bin/gate.sh read",no-pty,no-port-forwarding,no-agent-forwarding,no-X11-forwarding %s\n' \
    "$(cat /etc/git-server/read.pub)"
  printf 'command="/usr/local/bin/gate.sh write",no-pty,no-port-forwarding,no-agent-forwarding,no-X11-forwarding %s\n' \
    "$(cat /etc/git-server/write.pub)"
} > /home/git/.ssh/authorized_keys
chown -R git:git /home/git/.ssh
chmod 700 /home/git/.ssh
chmod 600 /home/git/.ssh/authorized_keys
exec /usr/sbin/sshd -D -e \
  -o HostKey=/etc/ssh/ssh_host_ed25519_key \
  -o PasswordAuthentication=no \
  -o KbdInteractiveAuthentication=no
