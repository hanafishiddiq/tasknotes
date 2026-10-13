#!/bin/bash
# Isolated Tasknotes Issue19 hover A/B, one-command native QA.
# --reuse-bundle skips rebuilding the already verified QA app.
set -euo pipefail
cd "$(dirname "$0")/../.."
# Never replace target/debug/tasknotes or any ordinary Tasknotes.app build.
QA_TARGET="$(pwd)/src-tauri/target/issue19-hover-native"
APP="$QA_TARGET/debug/bundle/macos/Tasknotes Issue19 Hover A B QA.app"
if [ "$#" -eq 0 ]; then
  node scripts/qa/prepare-issue19-hover-ab.mjs
  PATH="$HOME/.cargo/bin:$PATH" CARGO_TARGET_DIR="$QA_TARGET" CARGO_BUILD_JOBS=3 pnpm tauri build --debug \
    --bundles app --no-sign --ci \
    --config src-tauri/target/issue19-hover-ab/hover-ab.conf.json
elif [ "$1" != "--reuse-bundle" ]; then
  echo "Usage: bash scripts/qa/run-issue19-hover-ab.sh [--reuse-bundle]" >&2
  exit 2
fi
if [ ! -d "$APP" ]; then
  echo "Missing isolated QA bundle; run bash scripts/qa/run-issue19-hover-ab.sh without --reuse-bundle first." >&2
  exit 2
fi
BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print CFBundleIdentifier' "$APP/Contents/Info.plist")"
test "$BUNDLE_ID" = "store.adamaja.tasknotes.issue19hoverab"
DIR="$(python3 - <<'PY'
from pathlib import Path
import tempfile,sqlite3,os
marker=Path('src-tauri/target/.issue19-hover-data-dir')
root=Path(tempfile.gettempdir()).resolve()
if marker.exists():
    folder=Path(marker.read_text().strip()).resolve(strict=True)
else:
    folder=Path(tempfile.mkdtemp(prefix='tasknotes-hover-ab.',dir=root)).resolve()
    os.chmod(folder,0o700)
    marker.write_text(str(folder)+'\n')
assert folder.parent==root
assert folder.name.startswith('tasknotes-hover-ab.')
assert folder.is_dir() and not folder.is_symlink()
db=folder/'tasknotes.db'
if db.exists():
    assert not db.is_symlink()
    with sqlite3.connect(f'file:{db}?mode=ro',uri=True) as c:
        titles=[row[0] for row in c.execute('SELECT title FROM tasks')]
        assert all(t.startswith('__ISSUE19_HOVER_QA__') for t in titles), 'Refusing non-QA user tasks'
        assert c.execute('PRAGMA integrity_check').fetchone()[0]=='ok'
print(folder)
PY
)"
printf 'Starting synthetic-only Tasknotes Hover A/B QA app; no WebDriver or user data.\n'
export TASKNOTES_DEV_DATA_DIR="$DIR"
exec "$APP/Contents/MacOS/tasknotes"
