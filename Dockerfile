# Docker image for Intro to Embedded Rust video series

# Settings
ARG RUST_VERSION=1.85.0-bookworm
ARG USER=student
ARG WGET_ARGS="-q --show-progress --progress=bar:force:noscroll"

#-------------------------------------------------------------------------------
# Base Image and Dependencies

# Use the official Rust image as the base
FROM rust:${RUST_VERSION}

# Redeclare arguments after FROM
ARG TARGETARCH
ARG USER
ARG WGET_ARGS

# Check if the target architecture is either x86_64 (amd64) or arm64 (aarch64)
RUN if [ "$TARGETARCH" = "amd64" ] || [ "$TARGETARCH" = "arm64" ]; then \
        echo "Architecture $TARGETARCH is supported."; \
    else \
        echo "Unsupported architecture: $TARGETARCH"; \
        exit 1; \
    fi

# Set environment variables
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    USER=${USER} \
    USER_UID=1000 \
    USER_GID=1000

# Install system dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libudev-dev \
    libusb-1.0-0-dev \
    binutils-arm-none-eabi \
    gdb-multiarch \
    git \
    curl \
    wget \
    unzip \
    sudo \
    vim \
    nano \
    dos2unix

# Clean up APT when done
RUN apt-get clean && \
    apt-get autoclean && \
    apt-get autoremove -y && \
    rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd --gid $USER_GID $USER && \
    useradd --uid $USER_UID --gid $USER_GID -m $USER && \
    echo "$USER ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers

#-------------------------------------------------------------------------------
# Configure Rust

# Switch to user for installing Rust and tools
USER $USER
WORKDIR /home/$USER

# Add ARM Cortex-M targets for RP2040/RP2350
RUN rustup target add thumbv6m-none-eabi && \
    rustup target add thumbv8m.main-none-eabihf

# Install essential tools for embedded development as the student user
RUN cargo install \
    cargo-binutils \
    flip-link \
    elf2uf2-rs

# Install linting, formatting, and inspection tools
RUN rustup component add \
    clippy \
    rustfmt \
    llvm-tools

#-------------------------------------------------------------------------------
# Install Pico Tools

# Switch to root
USER root

# Install picotool
RUN case "${TARGETARCH}" in \
        amd64) PICOTOOL_URL="https://github.com/raspberrypi/pico-sdk-tools/releases/download/v2.2.0-1/picotool-2.2.0-a4-x86_64-lin.tar.gz" ;; \
        arm64) PICOTOOL_URL="https://github.com/raspberrypi/pico-sdk-tools/releases/download/v2.2.0-1/picotool-2.2.0-a4-aarch64-lin.tar.gz" ;; \
        *) echo "Unsupported architecture: ${TARGETARCH}" && exit 1 ;; \
    esac && \
    cd /tmp && \
    wget ${WGET_ARGS} -O picotool.tar.gz "${PICOTOOL_URL}" && \
    tar -xzf picotool.tar.gz && \
    cp picotool/picotool /usr/local/bin/ && \
    chmod +x /usr/local/bin/picotool && \
    rm -rf picotool.tar.gz picotool-*

#-------------------------------------------------------------------------------
# Entrypoint

# Copy entrypoint script
USER root
COPY .scripts/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh && \
    dos2unix /usr/local/bin/entrypoint.sh

# Switch back to the non-root user
USER $USER

# Set working directory
RUN mkdir -p /home/$USER/workspace && \
    chown -R $USER:$USER /home/$USER/workspace
WORKDIR /home/$USER/workspace

#Run entrypoint script
CMD ["/usr/local/bin/entrypoint.sh"]
