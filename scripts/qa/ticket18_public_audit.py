#!/usr/bin/env python3
"""Audit the public repository, its whole history, for what must never be there.

Usage: ticket18_public_audit.py OUT_JSON
Reports findings by file and line, never the matched secret itself.
"""
import json
import os
import re
import subprocess
import sys
import tempfile

FORBIDDEN_FILES = re.compile(
    r"\.(db|sqlite|sqlite3|db-wal|db-shm|sqlite-wal|sqlite-shm|bak|p12|pem|key|p8|mobileprovision"
    r"|mp3|wav|aiff?|m4a|flac|ogg|aac)$|(^|/)(\.env(\..*)?|credentials\.json|ai-access\.json)$",
    re.IGNORECASE,
)
ALLOWED_FILES = re.compile(r"\.env\.example$")
# No \\b: `git log -G` on Apple's git silently matches nothing with it. `main`
# proves on a throwaway repository that every pattern finds a planted fake.
SECRETS = {
    "GitHub token": (r"gh[pousr]_[A-Za-z0-9]{30,}", "ghp_" + "a1" * 18),
    "Anthropic key": (r"sk-ant-[A-Za-z0-9_-]{20,}", "sk-ant-" + "b2" * 12),
    "OpenAI-style key": (r"sk-[A-Za-z0-9]{30,}", "sk-" + "c3" * 18),
    "AWS key": (r"AKIA[0-9A-Z]{16}", "AKIA" + "D4" * 8),
    "private key block": (r"-----BEGIN [A-Z ]*PRIVATE KEY-----", "-----BEGIN RSA PRIVATE " + "KEY-----"),
    "Slack token": (r"xox[abprs]-[A-Za-z0-9-]{10,}", "xoxb-" + "e5" * 8),
}
PERSONAL = {
    "email address": r"[A-Za-z0-9._%+-]+@(gmail|yahoo|outlook|hotmail|icloud)\.com",
    "home directory": r"/Users/[a-z][a-z0-9_-]+/",
}


def git(*args, cwd=None):
    return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, errors="replace", check=True).stdout


def prove_patterns_work():
    """Every pattern must find its planted fake here, or an empty result proves nothing."""
    with tempfile.TemporaryDirectory(prefix="tasknotes-audit-") as repo:
        git("init", "-q", cwd=repo)
        with open(os.path.join(repo, "planted.txt"), "w") as handle:
            handle.write("\n".join(fake for _, fake in SECRETS.values()) + "\n")
        git("add", ".", cwd=repo)
        git("-c", "user.name=audit", "-c", "user.email=audit@example.invalid", "commit", "-qm", "planted", cwd=repo)
        for name, (pattern, _) in SECRETS.items():
            if not git("log", "--format=%h", "-G", pattern, cwd=repo).split():
                sys.exit(f"The {name} pattern does not work with this git; the audit would prove nothing")


def main():
    out = sys.argv[1]
    prove_patterns_work()
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
    for name, (pattern, _) in SECRETS.items():
        commits = git("log", "--all", "--format=%h", "-G", pattern).split()
        report["secrets_in_any_commit"][name] = commits
    for name, pattern in PERSONAL.items():
        now = subprocess.run(["git", "grep", "-nIE", pattern, "HEAD"], capture_output=True, text=True, errors="replace").stdout
        lines = [":".join(line.split(":")[1:3]) for line in now.splitlines()]
        report["personal_data_now"][name] = {"count": len(lines), "where": lines[:40]}
        history = set(git("log", "--all", "--format=%h", "-G", pattern).split())
        report["personal_data_in_history_only"][name] = {"commits_touching": len(history)}
    # Commit metadata is public too. Counted, never printed.
    emails = git("log", "--all", "--format=%ae%n%ce").splitlines()
    private = [email for email in emails if "noreply" not in email and not email.endswith("@example.invalid")]
    report["private_emails_in_commit_metadata"] = {"distinct": len(set(private)), "occurrences": len(private)}
    with open(out, "w") as handle:
        json.dump(report, handle, indent=2)
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
