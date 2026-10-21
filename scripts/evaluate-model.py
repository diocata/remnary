#!/usr/bin/env python3
"""Run the hand-labeled SYNTHETIC semantic cases against an installed local model."""
import argparse
import json
import hashlib
import shutil
from pathlib import Path
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/debug/observr"))
    parser.add_argument("--model", default="qwen3.5:9b-q4_K_M")
    parser.add_argument("--endpoint", default="http://127.0.0.1:11435")
    parser.add_argument("--case", dest="case_ids", action="append", help="Run only a named case; repeat for multiple cases")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    fixture_bytes = (root/"tests/fixtures/model-semantic-evaluation.json").read_bytes()
    cases = json.loads(fixture_bytes)
    if args.case_ids:
        unknown = set(args.case_ids)-{case["id"] for case in cases}
        if unknown:
            parser.error("unknown case IDs: "+", ".join(sorted(unknown)))
        cases = [case for case in cases if case["id"] in args.case_ids]
    binary = args.binary.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="observr-synthetic-model-evaluation-") as work:
        path = Path(work)
        # Freeze the tested executable so concurrent local rebuilds cannot
        # change its identity between initialization, analysis and reporting.
        snapshot_binary = path/"observr-evaluation"
        shutil.copy2(binary, snapshot_binary)
        binary = snapshot_binary
        binary_digest = hashlib.sha256(binary.read_bytes()).hexdigest()
        data, key, source = path/"archive", path/"recovery.key", path/"conversations.json"
        documents = []
        for index, case in enumerate(cases):
            mapping = {}
            def message_id(position):
                return hashlib.sha256(f'observr-synthetic-message:{case["id"]}:{position}'.encode()).hexdigest()
            for position, message in enumerate(case["messages"]):
                node = message_id(position)
                mapping[node] = {"id": node,
                                 "parent": message_id(position-1) if position else None,
                                 "children": [message_id(position+1)] if position+1 < len(case["messages"]) else [],
                                 "message": {"id": node, "author": {"role": message["role"]},
                                             "create_time": 1700000000+index*100+position,
                                             "content": {"content_type": "text", "parts": [message["text"]]}, "metadata": {}}}
            documents.append({"id": case["id"], "title": "Synthetic semantic case: "+case["id"],
                              "create_time": 1700000000+index*100, "mapping": mapping})
        source.write_text(json.dumps(documents), encoding="utf-8")

        def run(arguments, archive=True):
            command = [str(binary)]
            if archive:
                command += ["--data-dir", str(data), "--key-file", str(key)]
            return subprocess.run(command+list(map(str, arguments)), capture_output=True, text=True)

        for arguments, archive in [(["key", "create", key], False), (["init"], True), (["import", source], True)]:
            result = run(arguments, archive)
            if result.returncode:
                raise RuntimeError(result.stderr)
        identities = {r["source_id"]: r["id"] for r in json.loads(run(["sources", "--json"]).stdout)}
        start = time.monotonic()
        result = run(["analyze", "--engine", "ollama", "--model", args.model, "--endpoint", args.endpoint])
        print(result.stdout.strip())
        if result.stderr:
            print(result.stderr.strip())
        records = json.loads(run(["discover", "--all", "--json", "--limit", "1000"]).stdout)
        report = []
        for case in cases:
            claims = [r["observation"] for r in records if any(e["conversation_id"] == identities[case["id"]] for e in r["observation"]["evidence"])]
            issues = []
            if result.returncode and (identities[case["id"]] in result.stderr or not any(identity in result.stderr for identity in identities.values())):
                issues.append("analysis did not complete for this source")
            forbidden_values = case.get("forbidden_values", [])
            if any(word.casefold() in (c["summary"]+" "+c["canonical_value"]).casefold() for c in claims for word in forbidden_values):
                issues.append("unsupported meaning added to the source")
            if case.get("empty") and claims:
                issues.append("expected no personal observation")
            if any(c["kind"] in case.get("forbidden", []) for c in claims):
                issues.append("forbidden classification")
            if any(c["decision_event"] in case.get("forbidden_events", []) for c in claims):
                issues.append("discussion became a settled choice")
            for required in case.get("required", []):
                if not any(c["kind"] == required["kind"] and ("events" not in required or c["decision_event"] in required["events"]) for c in claims):
                    issues.append("required grounded classification missing")
            report.append({"case": case["id"], "passed": not issues, "issues": issues,
                           "claims": [{"kind": c["kind"], "event": c["decision_event"], "summary": c["summary"]} for c in claims]})
        output = {"synthetic": True, "message_ids_opaque": True, "model": args.model, "seconds": round(time.monotonic()-start, 2),
                  "fixture_sha256": hashlib.sha256(fixture_bytes).hexdigest(), "binary_sha256": binary_digest,
                  "selected_cases": [case["id"] for case in cases],
                  "analysis_success": result.returncode == 0, "passed": sum(r["passed"] for r in report),
                  "total": len(report), "cases": report}
        print(json.dumps(output, ensure_ascii=False, indent=2))
        if result.returncode or not all(r["passed"] for r in report):
            raise SystemExit(1)


if __name__ == "__main__":
    main()
