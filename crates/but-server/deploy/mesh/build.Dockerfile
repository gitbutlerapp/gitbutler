# Builds but-server for the mesh.but.dev box: Amazon Linux 2023 on Graviton (linux/arm64), so the
# binary links against the box's own glibc. Used by release.sh, which mounts the repository.
FROM amazonlinux:2023

RUN dnf install -y gcc gcc-c++ make cmake clang perl pkgconf-pkg-config openssl-devel git tar gzip \
    && dnf clean all

# The toolchain itself comes from rust-toolchain.toml when cargo first runs in the repository.
RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
ENV PATH=/root/.cargo/bin:$PATH
