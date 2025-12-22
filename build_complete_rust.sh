#!/bin/bash
# Wrapper script for backward compatibility with the original build_complete.sh

# Forward all arguments to the Rust binary
exec "$(dirname "$0")/target/release/build_complete" "$@"
