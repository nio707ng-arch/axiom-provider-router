#!/bin/sh
# Build the Darial OCI image with Apple Container. Run from the repository root.
set -eu
cd "$(dirname "$0")/../.."
container build --tag darial:0.1.0 --file deploy/docker/Dockerfile .
echo "built darial:0.1.0"
