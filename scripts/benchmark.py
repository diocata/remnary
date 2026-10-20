#!/usr/bin/env python3
"""Reproducible synthetic macOS archive benchmark; never reads real histories."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/release/observr"))
    parser.add_argument("--conversations", type=int, default=2000)
    parser.add_argument("--messages", type=int, default=16)
    parser.add_argument("--vault", action="store_true")
    args = parser.parse_args()
    if not (1 <= args.conversations <= 10000 and 1 <= args.messages <= 100):
        parser.error("use 1..10000 conversations and 1..100 messages")
    binary = args.binary.resolve(strict=True)
    stages = []
    with tempfile.TemporaryDirectory(prefix="observr-synthetic-benchmark-") as work:
        root = Path(work)
        source = root / "conversations.json"
        data = root / "archive"
        key = root / "recovery.key"
        conversations = []
        for index in range(args.conversations):
            mapping = {}
            for position in range(args.messages):
                message_id = f"message-{position}"
                text = ("Keep implementations simple. " if position == 0 else "")
                text += f"Synthetic benchmarkneedle conversation {index} message {position}. "
                text += "Synthetic context for archive performance, without personal information. " * 3
                mapping[message_id] = {
                    "id": message_id,
                    "parent": f"message-{position-1}" if position else None,
                    "children": [f"message-{position+1}"] if position+1 < args.messages else [],
                    "message": {"id": message_id, "author": {"role": "user" if position % 2 == 0 else "assistant"},
                                "create_time": 1700000000 + index*1200 + position*10,
                                "content": {"content_type": "text", "parts": [text]}, "metadata": {}},
                }
            conversations.append({"id": f"synthetic-{index}", "title": f"Synthetic benchmark {index}",
                                  "create_time": 1700000000+index*1200,
                                  "update_time": 1700000000+index*1200+args.messages*10,
                                  "mapping": mapping})
        source.write_text(json.dumps(conversations), encoding="utf-8")
        del conversations

        def run(stage, arguments, archive=True):
            command = [str(binary)]
            if archive:
                command += ["--data-dir", str(data), "--key-file", str(key)]
            command += list(map(str, arguments))
            start = time.monotonic()
            result = subprocess.run(["/usr/bin/time", "-l"] + command, text=True, capture_output=True)
            elapsed = time.monotonic() - start
            if result.returncode:
                raise RuntimeError(f"{stage} failed: {result.stderr}")
            match = re.search(r"(\d+)\s+maximum resident set size", result.stderr)
            stages.append({"stage": stage, "seconds": round(elapsed, 4),
                           "peak_rss_mib": round(int(match.group(1))/1048576, 2) if match else None})
            print(f"{stage}: {elapsed:.3f}s", flush=True)
            return result.stdout

        run("key", ["key", "create", key], archive=False)
        run("init", ["init"])
        run("import", ["import", source])
        repeated = run("unchanged_import", ["import", source])
        assert f"{args.conversations} unchanged" in repeated
        run("rules_analysis", ["analyze"])
        cached = run("cached_analysis", ["analyze"])
        assert "Analyzed 0 conversations" in cached
        for attempt in range(5):
            run(f"search_{attempt+1}", ["search", "benchmarkneedle", "--limit", "10"])
        run("discover", ["discover", "--json"])
        if args.vault:
            run("vault_sync", ["vault", "sync", root / "vault"])
            unchanged = run("unchanged_vault", ["vault", "sync", root / "vault"])
            assert "0 new or updated" in unchanged
        run("backup", ["backup", root / "backup.sqlite3"])
        doctor = json.loads(run("doctor", ["doctor"]))
        assert doctor["database_integrity"] and doctor["foreign_keys_valid"]
        report = {"synthetic": True, "conversations": args.conversations,
                  "messages": args.conversations*args.messages, "source_mib": round(source.stat().st_size/1048576, 2),
                  "database_mib": round((data/"observr.sqlite3").stat().st_size/1048576, 2),
                  "doctor": doctor, "stages": stages}
        print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
