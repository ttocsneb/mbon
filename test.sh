#!/bin/bash

cargo test --no-default-features --features sync
cargo test --no-default-features --features async-tokio
