# SPDX-FileCopyrightText: 2026 Daniel Vrátil <me@dvratil.cz>
#
# SPDX-License-Identifier: GPL-3.0-or-later

# Clareon build environment
# Provides Rust, C++, Qt6, and KDE Frameworks 6 toolchains
# along with all QML modules needed at runtime.
#
# Basic usage:
#   docker build -t clareon-build .
#
# If building behind an HTTPS-intercepting proxy, provide the proxy CA
# certificate as a build secret:
#   docker build \
#     --secret id=proxy-ca,src=/path/to/proxy-ca.crt \
#     --build-arg HTTP_PROXY=http://proxy:port \
#     --build-arg HTTPS_PROXY=http://proxy:port \
#     -t clareon-build .

FROM fedora:41

# Install proxy CA certificate if provided via build secret, so that
# dnf and curl can verify SSL through HTTPS-intercepting proxies.
RUN --mount=type=secret,id=proxy-ca \
    if [ -f /run/secrets/proxy-ca ]; then \
        cp /run/secrets/proxy-ca /etc/pki/ca-trust/source/anchors/proxy-ca.crt && \
        update-ca-trust extract; \
    fi

# Install build tools, Qt6, KDE Frameworks 6, and system libraries
RUN dnf install -y --setopt=install_weak_deps=False --nodocs \
    cmake \
    ninja-build \
    gcc \
    gcc-c++ \
    make \
    git \
    pkgconf-pkg-config \
    qt6-qtbase-devel \
    qt6-qtdeclarative-devel \
    extra-cmake-modules \
    kf6-kirigami-devel \
    kf6-kitemmodels-devel \
    qt6-qtquickcontrols2 \
    sqlite-devel \
    openssl-devel \
    dbus-devel \
    perl-FindBin \
    lld \
    && dnf clean all

# Install Rust via rustup (need recent stable for edition 2024)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /src
