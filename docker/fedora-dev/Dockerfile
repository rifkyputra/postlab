FROM fedora:41

# Build deps + runtime tools postlab uses locally
RUN dnf install -y --setopt=install_weak_deps=False \
        gcc pkg-config openssl-devel sqlite-devel \
        procps-ng util-linux iproute hostname coreutils \
        curl systemd && \
    dnf clean all && \
    # Remove units that don't work inside containers
    rm -f /lib/systemd/system/multi-user.target.wants/getty.target \
          /lib/systemd/system/multi-user.target.wants/systemd-ask-password-wall.path

# Rust toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --no-modify-path
ENV PATH="/root/.cargo/bin:/workspace/target/debug:$PATH"

WORKDIR /workspace

# Systemd as PID 1 so postlab can exercise systemctl calls
STOPSIGNAL SIGRTMIN+3
CMD ["/usr/sbin/init"]
