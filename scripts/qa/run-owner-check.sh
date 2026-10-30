#!/bin/sh
# Build and open "Tasknotes QA" for the owner's check. It has its own identifier,
# so it keeps its data in its own folder and can never open the real database.
# Pass --fresh to start again from an empty first launch, or --large to start
# with 50,000 synthetic Tasks and 10,000 synthetic Notes (takes about 5 minutes).
set -eu
root=$(cd "$(dirname "$0")/../.." && pwd)
identifier="store.adamaja.tasknotes.ownercheck"
data="$HOME/Library/Application Support/$identifier"
cd "$root"
if [ "${1:-}" = "--fresh" ] || [ "${1:-}" = "--large" ]; then
  rm -rf "$data" "$HOME/Library/WebKit/$identifier"
fi
if [ "${1:-}" = "--large" ]; then
  (cd crates/tasknotes-bench && cargo run --release -- seed "$data")
fi
pnpm install --frozen-lockfile
pnpm tauri build --bundles app \
  --config "{\"identifier\":\"$identifier\",\"productName\":\"Tasknotes QA\"}"
(cd crates/tasknotes-cli && cargo build --release)
(cd crates/tasknotes-mcp && cargo build --release)
open "src-tauri/target/release/bundle/macos/Tasknotes QA.app"
echo "Data folder: $data"
echo "CLI: $root/crates/tasknotes-cli/target/release/tasknotes --data-dir \"$data\" ..."
