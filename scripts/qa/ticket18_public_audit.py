#!/usr/bin/env python3
"""Audit the public repository, its whole history, for what must never be there.

Usage: ticket18_public_audit.py OUT_JSON
Reports findings by file and line, never the matched secret itself.
"""
import json
import re
import subprocess
import sys

FORBIDDEN_FILES = re.compile(
    r"\.(db|sqlite|sqlite3|db-wal|db-shm|sqlite-wal|sqlite-shm|bak|p12|pem|key|p8|mobileprovision"
    r"|mp3|wav|aiff?|m4a|flac|ogg|aac)$|(^|/)(\.env(\..*)?|credentials\.json|ai-access\.json)$",
    re.IGNORECASE,
)
ALLOWED_FILES = re.compile(r"\.env\.example$")
SECRETS = {
    "GitHub token": r"gh[pousr]_[A-Za-z0-9]{30,}",
    "Anthropic key": r"sk-ant-[A-Za-z0-9_-]{20,}",
    "OpenAI-style key": r"\bsk-[A-Za-z0-9]{30,}\b",
    "AWS key": r"\bAKIA[0-9A-Z]{16}\b",
    "private key block": r"-----BEGIN [A-Z ]*PRIVATE KEY-----",
    "Slack token": r"\bxox[abprs]-[A-Za-z0-9-]{10,}",
}
PERSONAL = {
    "email address": r"[A-Za-z0-9._%+-]+@(gmail|yahoo|outlook|hotmail|icloud)\.com",
    "home directory": r"/Users/[a-z][a-z0-9_-]+/",
}


def git(*args):
    return subprocess.run(["git", *args], capture_output=True, text=True, errors="replace", check=True).stdout


def main():
    out = sys.argv[1]
    every_path = sorted({
        line.split(" ", 1)[1]
        for line in git("rev-list", "--all", "--objects").splitlines()
        if " " in line
    })
    forbidden = [p for p in every_path if FORBIDDEN_FILES.search(p) and not ALLOWED_FILES.search(p)]
    current = git("ls-files").splitlines()
    report = {
        "paths_ever_committed": len(every_path),
        "forbidden_files_ever_committed": forbidden,
        "forbidden_files_now": [p for p in current if p in forbidden],
        "largest_files_now": [],
        "secrets_in_any_commit": {},
        "personal_data_now": {},
        "personal_data_in_history_only": {},
    }
    sizes = []
    for path in current:
        try:
            sizes.append((int(git("cat-file", "-s", f"HEAD:{path}")), path))
        except subprocess.CalledProcessError:
            pass
    report["largest_files_now"] = [{"bytes": s, "path": p} for s, p in sorted(sizes, reverse=True)[:8]]
    for name, pattern in SECRETS.items():
        commits = git("log", "--all", "--format=%h", "-G", pattern).split()
        report["secrets_in_any_commit"][name] = commits
    for name, pattern in PERSONAL.items():
        now = subprocess.run(["git", "grep", "-nIE", pattern, "HEAD"], capture_output=True, text=True, errors="replace").stdout
        lines = [":".join(line.split(":")[1:3]) for line in now.splitlines()]
        report["personal_data_now"][name] = {"count": len(lines), "where": lines[:40]}
        history = set(git("log", "--all", "--format=%h", "-G", pattern).split())
        report["personal_data_in_history_only"][name] = {"commits_touching": len(history)}
    with open(out, "w") as handle:
        json.dump(report, handle, indent=2)
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
