# The remote `run.sh` serves the corpus from: sshd and git, nothing else.
#
# It stands in for a git host's deploy keys. The two keys the overlay mounts are authorized here
# with a forced command, and `gate.sh` lets the read key fetch and refuses it a push. Without that
# a run would pass with the two keys swapped, and the test would not know.
#
# The login shell is `sh`, not `git-shell`: sshd runs a forced command through it, and git-shell
# refuses any command but git's own. Every key here has a forced command, so no key gets a shell.
FROM alpine:3.22
RUN apk add --no-cache git openssh-server \
 && adduser -D -h /home/git -s /bin/sh git \
 && sed -i 's/^git:!/git:*/' /etc/shadow \
 && mkdir -p /srv/git && chown git:git /srv/git
COPY gate.sh entrypoint.sh /usr/local/bin/
EXPOSE 22
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
