#!/bin/bash
set -e

echo "Building catalyst..."
cargo build --release

echo ""
echo "Installing catalyst to /usr/local/bin..."
sudo cp target/release/catalyst /usr/local/bin/

echo ""
echo "Catalyst installed successfully!"
echo "You can now run 'catalyst' from anywhere."
