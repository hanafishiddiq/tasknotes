#!/usr/bin/env python3
"""Measure the CLI and MCP against a seeded dataset: page time, the peak memory
of one process, and that a whole list arrives only page by page.

Usage: ticket17_ai_paging.py TASKNOTES_BIN MCP_BIN DATASET_DIR OUT_JSON
"""
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time


def percentile(samples, percent):
    ordered = sorted(samples)
    rank = max(1, -(-len(ordered) * percent // 100))
    return round(ordered[rank - 1], 2)


def timing(samples):
    return {
        "runs": len(samples),
        "p50_ms": percentile(samples, 50),
        "p95_ms": percentile(samples, 95),
        "max_ms": round(max(samples), 2),
    }


def cli(binary, data, *words):
    started = time.perf_counter()
    done = subprocess.run(
        ["/usr/bin/time", "-l", binary, "--data-dir", data, *words],
        capture_output=True, text=True,
    )
    spent = (time.perf_counter() - started) * 1000
    peak = re.search(r"(\d+)\s+maximum resident set size", done.stderr)
    return json.loads(done.stdout), spent, int(peak.group(1)) / (1024 * 1024) if peak else 0


def cli_page_timing(binary, data, *words, runs=30):
    samples, peak = [], 0
    for _ in range(runs):
        _, spent, rss = cli(binary, data, *words)
        samples.append(spent)
        peak = max(peak, rss)
    return {**timing(samples), "peak_rss_mb": round(peak, 1)}


def walk_cli(binary, data, noun, limit, max_pages):
    """Follow next_cursor; the most records any one answer held, and memory."""
    cursor, total, largest, peak, pages = None, 0, 0, 0, 0
    while pages < max_pages:
        words = [noun, "list", "--limit", str(limit)] + (["--cursor", cursor] if cursor else [])
        body, _, rss = cli(binary, data, *words)
        items = body.get("items", [])
        total, largest, peak = total + len(items), max(largest, len(items)), max(peak, rss)
        pages += 1
        cursor = body.get("next_cursor")
        if not cursor:
            break
    return {"pages": pages, "records": total, "largest_answer": largest,
            "peak_rss_mb": round(peak, 1), "finished": cursor is None}


class Mcp:
    def __init__(self, binary, data):
        self.child = subprocess.Popen(
            [binary, "--data-dir", data], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL, text=True,
        )
        self.next_id = 1
        self.send("initialize", {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": {"name": "ticket17", "version": "1"},
        })
        self.child.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
        self.child.stdin.flush()

    def send(self, method, params):
        message = {"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params}
        self.next_id += 1
        self.child.stdin.write(json.dumps(message) + "\n")
        self.child.stdin.flush()
        return json.loads(self.child.stdout.readline())

    def call(self, tool, **arguments):
        answer = self.send("tools/call", {"name": tool, "arguments": arguments})
        return answer["result"]["structuredContent"]

    def rss_mb(self):
        out = subprocess.run(["ps", "-o", "rss=", "-p", str(self.child.pid)], capture_output=True, text=True).stdout
        return int(out.strip() or 0) / 1024

    def close(self):
        self.child.stdin.close()
        self.child.wait(timeout=10)


def mcp_timing(mcp, tool, runs=100, **arguments):
    samples = []
    for _ in range(runs):
        started = time.perf_counter()
        mcp.call(tool, **arguments)
        samples.append((time.perf_counter() - started) * 1000)
    return timing(samples)


def walk_mcp(mcp, tool, limit, max_pages):
    cursor, total, largest, pages, peak = None, 0, 0, 0, 0
    while pages < max_pages:
        arguments = {"limit": limit}
        if cursor:
            arguments["cursor"] = cursor
        body = mcp.call(tool, **arguments)
        items = body.get("items", [])
        total, largest = total + len(items), max(largest, len(items))
        pages += 1
        peak = max(peak, mcp.rss_mb())
        cursor = body.get("next_cursor")
        if not cursor:
            break
    return {"pages": pages, "records": total, "largest_answer": largest,
            "peak_rss_mb": round(peak, 1), "finished": cursor is None}


def main():
    tasknotes, mcp_bin, dataset, out = sys.argv[1:5]
    if not os.path.exists(os.path.join(dataset, "synthetic-dataset.json")):
        sys.exit(f"{dataset} holds no synthetic dataset")
    report = {}
    with tempfile.TemporaryDirectory(prefix="tasknotes-ticket17-") as work:
        data = os.path.join(work, "data")
        shutil.copytree(dataset, data)
        with open(os.path.join(data, "ai-access.json"), "w") as handle:
            json.dump({"schema_version": 1, "cli": True, "mcp": True}, handle)
        # The first command of the day takes a backup; time the commands after it.
        first = cli(tasknotes, data, "task", "list", "--limit", "1")
        report["cli_first_command_with_backup_ms"] = round(first[1], 1)
        report["cli"] = {
            "task list, first page": cli_page_timing(tasknotes, data, "task", "list"),
            "task list, common word": cli_page_timing(tasknotes, data, "task", "list", "--text", "quarterly"),
            "task list, one letter": cli_page_timing(tasknotes, data, "task", "list", "--text", "b"),
            "note list, common word": cli_page_timing(tasknotes, data, "note", "list", "--text", "meeting"),
        }
        over = subprocess.run([tasknotes, "--data-dir", data, "task", "list", "--limit", "100000"],
                              capture_output=True, text=True)
        report["cli_limit_over_the_maximum"] = {
            "exit": over.returncode,
            "code": json.loads(over.stdout).get("error", {}).get("code"),
        }
        report["cli_walk_tasks"] = walk_cli(tasknotes, data, "task", 100, 600)
        report["cli_walk_notes"] = walk_cli(tasknotes, data, "note", 100, 120)
        mcp = Mcp(mcp_bin, data)
        report["mcp"] = {
            "task_list, first page": mcp_timing(mcp, "task_list"),
            "task_list, common word": mcp_timing(mcp, "task_list", text="quarterly"),
            "task_list, one letter": mcp_timing(mcp, "task_list", text="b"),
            "note_list, common word": mcp_timing(mcp, "note_list", text="meeting"),
        }
        over = mcp.call("task_list", limit=100000)
        report["mcp_limit_over_the_maximum"] = over.get("error", {}).get("code")
        report["mcp_walk_tasks"] = walk_mcp(mcp, "task_list", 100, 600)
        report["mcp_walk_notes"] = walk_mcp(mcp, "note_list", 100, 120)
        mcp.close()
    with open(out, "w") as handle:
        json.dump(report, handle, indent=2)
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
