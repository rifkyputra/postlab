FROM fedora:39

RUN dnf install -y --setopt=install_weak_deps=False \
        openssh-server sudo procps-ng iproute && \
    dnf clean all && \
    ssh-keygen -A && \
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
        -e 's/#PasswordAuthentication yes/PasswordAuthentication no/' \
        /etc/ssh/sshd_config

COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

EXPOSE 22
ENTRYPOINT ["/entrypoint.sh"]
