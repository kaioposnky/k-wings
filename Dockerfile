FROM alpine:latest

RUN apk add --no-cache ca-certificates coreutils curl btrfs-progs xfsprogs-extra zfs restic && \
	update-ca-certificates

# Add k-wings and entrypoint
ARG TARGETPLATFORM
COPY .docker/${TARGETPLATFORM#linux/}/k-wings /usr/bin/k-wings

ENV OCI_CONTAINER=official

ENTRYPOINT ["/usr/bin/k-wings"]
