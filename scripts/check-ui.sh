#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo build
orbit_test_dir=$(mktemp -d)
trap 'rm -rf "$orbit_test_dir"' EXIT
QT_FORCE_STDERR_LOGGING=1 QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
    ORBIT_CONFIG_DIR="$orbit_test_dir/config" \
    timeout 20s target/debug/orbit --demo --ui-test > "$orbit_test_dir/ui.log" 2>&1
cat "$orbit_test_dir/ui.log"
if ! grep -q 'ORBIT_UI_TEST_PASS' "$orbit_test_dir/ui.log"; then
    echo 'UI checks did not complete' >&2
    exit 1
fi
if grep -E 'ORBIT_UI_TEST_FAIL|ReferenceError|TypeError|Binding loop|Cannot assign|Unable to assign' "$orbit_test_dir/ui.log"; then
    exit 1
fi
test ! -e "$orbit_test_dir/config/settings.json"
