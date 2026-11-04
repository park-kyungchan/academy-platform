#!/usr/bin/env bash
# Source after cargo build --workspace --bins --examples --locked. The caller
# selects a disposable synthetic root; every invocation gets a unique directory.
# shellcheck shell=bash
set -euo pipefail
rust_fixture_parent="${1:?supply disposable synthetic fixture parent}"
mkdir -p "$rust_fixture_parent"
rust_fixture_root="$(mktemp -d "$rust_fixture_parent/run.XXXXXX")"
export S09_NATIVE_FIXTURE_ROOT="$rust_fixture_root"
export S10_TEST_ROOT="$rust_fixture_root"
export S11_TEST_ROOT="$rust_fixture_root"
export S11_INTAKE_TEST_ROOT="$rust_fixture_root"
export S11_ERROR_TEST_ROOT="$rust_fixture_root"
export S11_PARENT_GET_TEST_ROOT="$rust_fixture_root"
export S11_HISTORY_TEST_ROOT="$rust_fixture_root"
export S11_BUSINESS_TEST_ROOT="$rust_fixture_root"
rust_target_root="$(realpath "${CARGO_TARGET_DIR:-target}")"
export S11_MCP_BINARY="$rust_target_root/debug/edu-academy-mcp"
export S11_STDIO_PEER="$rust_target_root/debug/examples/s11-stdio-peer"
export S11_PARENT_GET_CHILD_EXECUTABLE="$rust_target_root/debug/examples/fictional_dailyrecord_endpoint"
export S11_HISTORY_CHILD_EXECUTABLE="$rust_target_root/debug/examples/fictional_paper_endpoint"
S11_PARENT_GET_CHILD_SHA256="$(sha256sum "$S11_PARENT_GET_CHILD_EXECUTABLE")"
export S11_PARENT_GET_CHILD_SHA256="${S11_PARENT_GET_CHILD_SHA256%% *}"
S11_HISTORY_CHILD_SHA256="$(sha256sum "$S11_HISTORY_CHILD_EXECUTABLE")"
export S11_HISTORY_CHILD_SHA256="${S11_HISTORY_CHILD_SHA256%% *}"
