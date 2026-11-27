#!/usr/bin/env bash
# Source after cargo build --bins --examples --locked. The caller selects a
# disposable synthetic root; every invocation gets a unique directory.
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
