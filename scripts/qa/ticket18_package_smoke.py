#!/usr/bin/env python3
"""Inspect a packaged QA build and smoke-test its launches without a screen.

Usage: ticket18_package_smoke.py QA_APP_BUNDLE TASKNOTES_CLI OUT_JSON

The bundle must carry a QA identifier, so it has its own data folder: a build
with the real identifier would open and upgrade the person's real database.
"""
import json
import os
import plistlib
import re
import shutil
import signal
import subprocess
import sys
import time

QA_SUFFIX = ".ticket18qa"
FORBIDDEN = re.compile(r"\.(db|sqlite3?|mp3|wav|aiff?|m4a|flac|ogg|aac|map|env)$", re.IGNORECASE)


def run(*args):
    return subprocess.run(args, capture_output=True, text=True, errors="replace")


def inspect(bundle, binary):
    files, total = [], 0
    for folder, _, names in os.walk(bundle):
        for name in names:
            path = os.path.join(folder, name)
            size = os.path.getsize(path)
            total += size
            files.append(os.path.relpath(path, bundle))
    with open(binary, "rb") as handle:
        content = handle.read()
    home_paths = sorted(set(re.findall(rb"/Users/[A-Za-z0-9_.-]+/[A-Za-z0-9_./-]{0,60}", content)))
    signature = run("codesign", "-dv", bundle).stderr
    return {
        "files": len(files),
        "size_mb": round(total / (1024 * 1024), 1),
        "forbidden_files": [f for f in files if FORBIDDEN.search(f)],
        "contains_probe": b"qa_probe_active" in content or b"TASKNOTES_QA_REPORT" in content,
        "accepts_a_data_folder_from_the_environment": b"TASKNOTES_DEV_DATA_DIR" in content,
        "home_paths_in_binary": {
            "count": len(home_paths),
            # The folder names are from building here; the account name stays out of the report.
            "examples": [re.sub(r"^/Users/[^/]+", "~", p.decode(errors="replace")) for p in home_paths[:5]],
        },
        "signature": [line for line in signature.splitlines() if line.startswith(("Signature", "TeamIdentifier"))],
    }


def sockets(pid):
    out = run("lsof", "-nP", "-a", "-p", str(pid), "-i").stdout
    return [line.split(None, 8)[-1] for line in out.splitlines()[1:]]


def launch(binary, wait=8):
    child = subprocess.Popen([binary], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
    time.sleep(wait)
    alive = child.poll() is None
    open_sockets = sockets(child.pid) if alive else None
    if alive:
        child.send_signal(signal.SIGTERM)
        try:
            child.wait(timeout=15)
        except subprocess.TimeoutExpired:
            child.kill()
    return {"stayed_running": alive, "network_sockets": open_sockets}


def main():
    bundle, cli, out = sys.argv[1:4]
    with open(os.path.join(bundle, "Contents", "Info.plist"), "rb") as handle:
        info = plistlib.load(handle)
    identifier = info["CFBundleIdentifier"]
    if not identifier.endswith(QA_SUFFIX):
        sys.exit(f"{identifier} is not a QA identifier; launching it would open real data")
    binary = os.path.join(bundle, "Contents", "MacOS", info["CFBundleExecutable"])
    data = os.path.expanduser(f"~/Library/Application Support/{identifier}")
    if os.path.exists(data):
        sys.exit(f"{data} already exists; remove it first so this is a first launch")
    report = {"identifier": identifier, "version": info.get("CFBundleShortVersionString"), "bundle": inspect(bundle, binary)}
    try:
        report["first_launch"] = launch(binary)
        database = os.path.join(data, "tasknotes.db")
        report["database_created_on_first_launch"] = os.path.exists(database)
        backups = os.path.join(data, "backups")
        report["backup_files_after_first_launch"] = (
            len([n for n in os.listdir(backups) if n.endswith(".json")]) if os.path.isdir(backups) else 0
        )
        with open(os.path.join(data, "ai-access.json"), "w") as handle:
            json.dump({"schema_version": 1, "cli": True}, handle)
        def task_list():
            done = run(cli, "--data-dir", data, "task", "list")
            return json.loads(done.stdout)
        made = json.loads(run(cli, "--data-dir", data, "task", "create", "--title", "Synthetic smoke task").stdout)
        report["task_created_between_launches"] = made.get("ok")
        report["second_launch"] = launch(binary)
        titles = [item["title"] for item in task_list().get("items", [])]
        report["task_survives_second_launch"] = titles == ["Synthetic smoke task"]
    finally:
        shutil.rmtree(data, ignore_errors=True)
        for leftover in (f"~/Library/WebKit/{identifier}", f"~/Library/Caches/{identifier}",
                         f"~/Library/HTTPStorages/{identifier}", f"~/Library/Saved Application State/{identifier}.savedState"):
            shutil.rmtree(os.path.expanduser(leftover), ignore_errors=True)
    with open(out, "w") as handle:
        json.dump(report, handle, indent=2)
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
