FROM rust:1.75

WORKDIR /usr/src/myapp

# First, copy only the Cargo.toml and Cargo.lock to cache dependencies
COPY Cargo.toml Cargo.lock ./
# This dummy build helps to cache dependencies
RUN mkdir src/ \
    && echo "fn main() {println!(\"if you see this, the build broke\")}" > src/main.rs \
    && cargo build --release \
    && rm -f target/release/deps/myapp*

# Now, copy the rest of your source code
COPY . .

# Build the actual application
RUN cargo install --path .

# The binary is now in ~/.cargo/bin, adjust the path if necessary
CMD ["/usr/local/cargo/bin/myapp"]
