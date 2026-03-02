FROM ubuntu:22.04

RUN apt-get update -qq && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        openssh-server sudo procps iproute2 && \
    rm -rf /var/lib/apt/lists/* && \
    mkdir /var/run/sshd && \
    useradd -m -s /bin/bash postlab && \
    echo 'postlab ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/postlab && \
    chmod 0440 /etc/sudoers.d/postlab && \
    mkdir -p /home/postlab/.ssh && \
    chown postlab:postlab /home/postlab/.ssh && \
    chmod 700 /home/postlab/.ssh && \
    usermod -p '*' postlab

# Harden sshd config for container use
RUN sed -i \
        -e 's/#PubkeyAuthentication yes/PubkeyAuthentication yes/' \
        -e 's/UsePAM yes/UsePAM no/' \
        -e 's/#PasswordAuthentication yes/PasswordAuthentication no/' \
        /etc/ssh/sshd_config

COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

EXPOSE 22
ENTRYPOINT ["/entrypoint.sh"]
