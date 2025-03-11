#!/bin/bash

set -e # Exit immediately if a command exits with a non-zero status

# Function to publish a crate and wait for it to be available
publish_crate() {
    local crate_name="$1"
    echo "Publishing $crate_name..."

    # Publish the crate
    cargo publish -p "$crate_name" --registry crates-io

    # Wait for the crate to be available on crates.io
    echo "Waiting for $crate_name to be available on crates.io..."
    sleep 3 # Initial delay to give crates.io time to process

    # You might need to adjust this timeout depending on crates.io processing time
    local max_attempts=10
    local attempt=1

    while [ $attempt -le $max_attempts ]; do
        echo "Checking if $crate_name is available (attempt $attempt/$max_attempts)..."

        # Try to fetch the crate info from crates.io API
        if curl -s "https://crates.io/api/v1/crates/$crate_name" | grep -q "\"name\":\"$crate_name\""; then
            echo "$crate_name is now available on crates.io!"
            return 0
        fi

        echo "$crate_name not yet available, waiting..."
        sleep 3
        ((attempt++))
    done

    echo "Warning: Timed out waiting for $crate_name to appear on crates.io"
    echo "Continuing with the next crate, but there might be dependency issues."
    return 1
}

# Main script execution
echo "Starting AIScript crates publication process..."

# Publish crates in dependency order
publish_crate "aiscript-lexer"
publish_crate "aiscript-directive"
publish_crate "aiscript-vm"
publish_crate "aiscript-runtime"
publish_crate "aiscript"

echo "Publication process completed!"
