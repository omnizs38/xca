#!/usr/bin/env python3
"""Reproducible cross-engine benchmark runner for XCA."""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
import platform
import random
import shutil
import statistics
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.request
import zipfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
BENCH = ROOT / "bench"
CACHE = ROOT / ".bench-cache"
LOCK_PATH = BENCH / "dependencies.lock.json"
CONFIG_PATH = BENCH / "config.json"
USER_AGENT = "xca-benchmark/1"


def load_json(path: Path) -> dict[str, Any]:
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def run_checked(command: list[str], *, cwd: Path | None = None, env: dict[str, str] | None = None) -> None:
    print("+", " ".join(command))
    subprocess.run(command, cwd=cwd, env=env, check=True)


def download(url: str, target: Path, expected: str, offline: bool) -> Path:
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.exists() and sha256(target) == expected:
        return target
    if offline:
        raise RuntimeError(f"missing cached file in offline mode: {target.name}")
    partial = target.with_suffix(target.suffix + ".part")
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(request, timeout=60) as source, partial.open("wb") as output:
                shutil.copyfileobj(source, output)
            if sha256(partial) != expected:
                raise RuntimeError(f"SHA-256 mismatch for {url}")
            partial.replace(target)
            return target
        except Exception:
            partial.unlink(missing_ok=True)
            if attempt == 2:
                raise
            time.sleep(2 ** attempt)
    raise AssertionError("unreachable")


def safe_destination(root: Path, name: str) -> Path:
    destination = (root / name).resolve()
    if destination != root.resolve() and root.resolve() not in destination.parents:
        raise RuntimeError(f"unsafe archive member: {name}")
    return destination


def extract(archive: Path, destination: Path) -> Path:
    marker = destination / ".complete"
    if marker.exists():
        return destination
    shutil.rmtree(destination, ignore_errors=True)
    destination.mkdir(parents=True)
    if zipfile.is_zipfile(archive):
        with zipfile.ZipFile(archive) as package:
            for member in package.infolist():
                safe_destination(destination, member.filename)
            package.extractall(destination)
    elif tarfile.is_tarfile(archive):
        with tarfile.open(archive) as package:
            for member in package.getmembers():
                safe_destination(destination, member.name)
            package.extractall(destination, filter="data")
    else:
        raise RuntimeError(f"unsupported archive: {archive}")
    marker.write_text("ok\n", encoding="utf-8")
    return destination


def platform_key() -> str:
    if os.name == "nt" and platform.machine().lower() in {"amd64", "x86_64"}:
        return "windows-x86_64"
    if os.name != "nt":
        return "unix"
    raise RuntimeError(f"unsupported platform: {platform.system()} {platform.machine()}")


def only_child(root: Path) -> Path:
    children = [path for path in root.iterdir() if path.name != ".complete"]
    return children[0] if len(children) == 1 and children[0].is_dir() else root


def setup_tool(name: str, spec: dict[str, Any], offline: bool) -> Path:
    variant = spec[platform_key()]
    archive = download(variant["url"], CACHE / "downloads" / Path(variant["url"]).name, variant["sha256"], offline)
    source = extract(archive, CACHE / "sources" / f"{name}-{spec['version']}-{platform_key()}")
    root = only_child(source)
    binary = root / variant["binary"]
    if binary.exists():
        return binary.resolve()
    if platform_key() != "unix":
        raise RuntimeError(f"binary not found after extraction: {binary}")
    jobs = str(max(1, os.cpu_count() or 1))
    if name == "zstd":
        run_checked(["make", "-j", jobs, "zstd-release"], cwd=root)
    elif name == "lz4":
        run_checked(["make", "-j", jobs, "lz4-release"], cwd=root)
    elif name == "xz":
        run_checked(["sh", "./configure", "--disable-shared", "--disable-doc", "--disable-scripts"], cwd=root)
        run_checked(["make", "-j", jobs], cwd=root)
    else:
        raise RuntimeError(f"unknown tool: {name}")
    if not binary.exists():
        raise RuntimeError(f"build did not produce {binary}")
    return binary.resolve()


def generate_synthetic(destination: Path) -> list[Path]:
    destination.mkdir(parents=True, exist_ok=True)
    rng = random.Random(0x584341)
    files: dict[str, bytes] = {}
    files["repeated-text.txt"] = (b"XCA reproducible benchmark line: status=active value=42\n" * 20000)
    files["random.bin"] = rng.randbytes(2_000_000)
    base = bytearray(rng.randbytes(512_000))
    snapshots = bytearray()
    for copy_index, changes in enumerate((0, 1, 16, 256, 4096)):
        current = base.copy()
        for change in range(changes):
            current[(change * 7919 + copy_index) % len(current)] ^= (change + copy_index) & 255
        snapshots.extend(current)
    files["near-duplicates.bin"] = bytes(snapshots)
    files["shifted-repeats.bin"] = b"".join(
        bytes([shift & 255]) * shift + bytes(range(251)) * 256 for shift in (1, 15, 255, 4095)
    )
    files["prefix-random.bin"] = b"structured-prefix:" * 32768 + rng.randbytes(1_000_000)
    files["random-prefix.bin"] = rng.randbytes(1_000_000) + b":structured-suffix" * 32768
    for name, data in files.items():
        path = destination / name
        if not path.exists() or sha256(path) != hashlib.sha256(data).hexdigest():
            path.write_bytes(data)
    return sorted(destination.iterdir())


def setup_corpora(suite: str, lock: dict[str, Any], offline: bool) -> list[Path]:
    corpora = generate_synthetic(CACHE / "corpora" / "synthetic")
    wanted = {"quick": {"quick"}, "full": {"quick", "full"}}[suite]
    for name, spec in lock["corpora"].items():
        if spec["suite"] not in wanted:
            continue
        archive = download(spec["url"], CACHE / "downloads" / Path(spec["url"]).name, spec["sha256"], offline)
        root = extract(archive, CACHE / "corpora" / name)
        corpora.extend(path for path in root.rglob("*") if path.is_file() and path.name != ".complete")
    return sorted(corpora)


def git_output(*args: str, cwd: Path = ROOT) -> str:
    return subprocess.check_output(["git", *args], cwd=cwd, text=True).strip()


def build_xca(ref: str | None = None) -> tuple[str, Path]:
    if ref is None:
        run_checked(["cargo", "build", "--release"], cwd=ROOT)
        return (git_output("rev-parse", "HEAD"), (ROOT / "target" / "release" / exe("xca")).resolve())
    commit = git_output("rev-parse", f"{ref}^{{commit}}")
    worktree = CACHE / "worktrees" / commit[:12]
    target = CACHE / "targets" / commit[:12]
    if not worktree.exists():
        worktree.parent.mkdir(parents=True, exist_ok=True)
        run_checked(["git", "worktree", "add", "--detach", str(worktree), commit], cwd=ROOT)
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(target)
    run_checked(["cargo", "build", "--release"], cwd=worktree, env=environment)
    return (commit, (target / "release" / exe("xca")).resolve())


def exe(name: str) -> str:
    return name + (".exe" if os.name == "nt" else "")


def command_for(engine: str, binary: Path, operation: str, source: Path, target: Path, profile: str) -> tuple[list[str], dict[str, str]]:
    env = os.environ.copy()
    threads = "1" if profile == "single" else str(max(1, os.cpu_count() or 1))
    if engine.startswith("xca"):
        env["XCA_THREADS"] = threads
        return ([str(binary), "compress" if operation == "encode" else "decompress", str(source), str(target)], env)
    if engine == "zstd":
        if operation == "encode":
            return ([str(binary), "-3", f"-T{threads}", "-q", "-f", str(source), "-o", str(target)], env)
        return ([str(binary), "-d", "-q", "-f", str(source), "-o", str(target)], env)
    if engine == "lz4":
        if operation == "encode":
            return ([str(binary), "-q", "-f", str(source), str(target)], env)
        return ([str(binary), "-q", "-d", "-f", str(source), str(target)], env)
    if engine == "xz":
        if operation == "encode":
            return ([str(binary), "-6", f"-T{threads}", "-c", str(source)], env)
        return ([str(binary), "-d", "-c", str(source)], env)
    raise RuntimeError(f"unknown engine: {engine}")


def invoke(engine: str, binary: Path, operation: str, source: Path, target: Path, profile: str) -> float:
    target.unlink(missing_ok=True)
    command, environment = command_for(engine, binary, operation, source, target, profile)
    started = time.perf_counter()
    if engine == "xz":
        with target.open("wb") as output:
            completed = subprocess.run(command, env=environment, stdout=output, stderr=subprocess.DEVNULL)
    else:
        completed = subprocess.run(command, env=environment, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    elapsed = time.perf_counter() - started
    if completed.returncode != 0:
        raise RuntimeError(f"{engine} {operation} failed with exit code {completed.returncode}")
    return elapsed


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    position = (len(ordered) - 1) * fraction
    low = int(position)
    high = min(low + 1, len(ordered) - 1)
    weight = position - low
    return ordered[low] * (1 - weight) + ordered[high] * weight


def tool_version(binary: Path) -> str:
    for args in (["--version"], ["-V"]):
        result = subprocess.run([str(binary), *args], text=True, capture_output=True)
        if result.returncode == 0:
            return (result.stdout or result.stderr).splitlines()[0].strip()
    return "unknown"


def write_results(output: Path, raw: list[dict[str, Any]], metadata: dict[str, Any]) -> None:
    output.mkdir(parents=True, exist_ok=True)
    fields = list(raw[0]) if raw else []
    with (output / "raw.csv").open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader(); writer.writerows(raw)
    groups: dict[tuple[str, str, str], list[dict[str, Any]]] = {}
    for row in raw:
        groups.setdefault((row["corpus"], row["engine"], row["profile"]), []).append(row)
    summary: list[dict[str, Any]] = []
    for (corpus, engine, profile), rows in sorted(groups.items()):
        enc = [float(row["encode_seconds"]) for row in rows]
        dec = [float(row["decode_seconds"]) for row in rows]
        size = int(rows[0]["source_bytes"])
        archive = int(rows[0]["archive_bytes"])
        summary.append({
            "corpus": corpus, "engine": engine, "profile": profile,
            "source_bytes": size, "archive_bytes": archive,
            "ratio_percent": round(archive * 100 / size, 4),
            "encode_median_ms": round(statistics.median(enc) * 1000, 3),
            "encode_iqr_ms": round((percentile(enc, .75) - percentile(enc, .25)) * 1000, 3),
            "encode_mib_s": round(size / 1048576 / statistics.median(enc), 2),
            "decode_median_ms": round(statistics.median(dec) * 1000, 3),
            "decode_iqr_ms": round((percentile(dec, .75) - percentile(dec, .25)) * 1000, 3),
            "decode_mib_s": round(size / 1048576 / statistics.median(dec), 2),
        })
    with (output / "summary.csv").open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(summary[0]) if summary else [])
        writer.writeheader(); writer.writerows(summary)
    (output / "metadata.json").write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    lines = ["# XCA benchmark report", "", f"Generated: {metadata['generated_at']}", "", "| Corpus | Engine | CPU profile | Ratio | Encode MiB/s | Decode MiB/s |", "|---|---|---:|---:|---:|---:|"]
    for row in summary:
        lines.append(f"| {row['corpus']} | {row['engine']} | {row['profile']} | {row['ratio_percent']:.2f}% | {row['encode_mib_s']:.2f} | {row['decode_mib_s']:.2f} |")
    lines += [
        "",
        "Raw timings and IQR values are in `raw.csv` and `summary.csv`. SHA-256 verification runs outside timed regions.",
        "LZ4 is single-threaded in both CPU profiles; XCA, Zstandard, and XZ receive the selected thread budget.",
        "These are end-to-end CLI and file-I/O measurements; use `xca bench` separately for the in-process codec path.",
    ]
    (output / "report.md").write_text("\n".join(lines) + "\n", encoding="utf-8")


def cmd_doctor(_: argparse.Namespace) -> None:
    required = ["git", "cargo"] + ([] if os.name == "nt" else ["make", "cc"])
    missing = [name for name in required if shutil.which(name) is None]
    print(f"Python: {sys.version.split()[0]}")
    print(f"Platform: {platform.platform()}")
    print(f"CPU count: {os.cpu_count() or 1}")
    for name in required:
        print(f"{name}: {shutil.which(name) or 'MISSING'}")
    if missing:
        raise SystemExit("missing required tools: " + ", ".join(missing))


def cmd_setup(args: argparse.Namespace) -> None:
    lock = load_json(LOCK_PATH)
    tools = {name: str(setup_tool(name, spec, args.offline)) for name, spec in lock["tools"].items()}
    commit, xca = build_xca()
    corpora = setup_corpora(args.suite, lock, args.offline)
    state = {"schema": 1, "suite": args.suite, "commit": commit, "tools": {"xca": str(xca), **tools}, "corpora": [str(path.resolve()) for path in corpora]}
    CACHE.mkdir(exist_ok=True)
    (CACHE / "state.json").write_text(json.dumps(state, indent=2) + "\n", encoding="utf-8")
    print(f"Ready: {len(corpora)} corpus files, {len(tools) + 1} engines")


def cmd_run(args: argparse.Namespace) -> None:
    state_path = CACHE / "state.json"
    if not state_path.exists():
        raise SystemExit("run setup first")
    state = load_json(state_path)
    config = load_json(CONFIG_PATH)[args.suite]
    iterations = args.iterations or int(config["iterations"])
    warmups = int(config["warmups"])
    profiles = [args.profile] if args.profile != "both" else list(config["profiles"])
    tools = {name: Path(path) for name, path in state["tools"].items()}
    if args.baseline:
        commit, binary = build_xca(args.baseline)
        tools[f"xca@{commit[:12]}"] = binary
    corpora = [Path(path) for path in state["corpora"]]
    corpora.extend(Path(path).resolve() for path in args.input)
    engines = list(tools)
    run_root = CACHE / "runs" / time.strftime("%Y%m%d-%H%M%S")
    work = run_root / "work"; work.mkdir(parents=True)
    raw: list[dict[str, Any]] = []
    rng = random.Random(0x584341)
    for corpus in corpora:
        if not corpus.is_file() or corpus.stat().st_size == 0:
            continue
        source_hash = sha256(corpus)
        source_size = corpus.stat().st_size
        for profile in profiles:
            combinations = engines[:]
            rng.shuffle(combinations)
            for engine in combinations:
                binary = tools[engine]
                safe = hashlib.sha256(str(corpus).encode()).hexdigest()[:12]
                archive = work / f"{safe}-{engine.replace('@','-')}-{profile}.arc"
                restored = work / f"{safe}-{engine.replace('@','-')}-{profile}.out"
                base_engine = "xca" if engine.startswith("xca") else engine
                for _ in range(warmups):
                    invoke(base_engine, binary, "encode", corpus, archive, profile)
                    invoke(base_engine, binary, "decode", archive, restored, profile)
                if sha256(restored) != source_hash:
                    raise RuntimeError(f"warm-up verification failed for {engine}: {corpus}")
                for iteration in range(1, iterations + 1):
                    encode_time = invoke(base_engine, binary, "encode", corpus, archive, profile)
                    archive_size = archive.stat().st_size
                    decode_time = invoke(base_engine, binary, "decode", archive, restored, profile)
                    if sha256(restored) != source_hash:
                        raise RuntimeError(f"verification failed for {engine}: {corpus}")
                    raw.append({"corpus": f"{corpus.parent.name}/{corpus.name}", "source_sha256": source_hash, "engine": engine, "profile": profile, "iteration": iteration, "source_bytes": source_size, "archive_bytes": archive_size, "encode_seconds": f"{encode_time:.9f}", "decode_seconds": f"{decode_time:.9f}"})
                print(f"done {corpus.name}: {engine} / {profile}")
    metadata = {"generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "git_commit": git_output("rev-parse", "HEAD"), "git_dirty": bool(git_output("status", "--porcelain")), "platform": platform.platform(), "python": sys.version, "cpu_count": os.cpu_count() or 1, "iterations": iterations, "warmups": warmups, "tools": {name: {"path": str(path), "sha256": sha256(path), "version": tool_version(path)} for name, path in tools.items()}}
    write_results(run_root, raw, metadata)
    print(run_root)


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    commands = result.add_subparsers(dest="command", required=True)
    doctor = commands.add_parser("doctor"); doctor.set_defaults(function=cmd_doctor)
    setup = commands.add_parser("setup"); setup.add_argument("--suite", choices=("quick", "full"), default="quick"); setup.add_argument("--offline", action="store_true"); setup.set_defaults(function=cmd_setup)
    run = commands.add_parser("run"); run.add_argument("--suite", choices=("quick", "full"), default="quick"); run.add_argument("--iterations", type=int); run.add_argument("--profile", choices=("single", "auto", "both"), default="both"); run.add_argument("--baseline"); run.add_argument("--input", action="append", default=[]); run.set_defaults(function=cmd_run)
    return result


def main() -> None:
    args = parser().parse_args()
    if getattr(args, "iterations", 1) is not None and getattr(args, "iterations", 1) < 1:
        raise SystemExit("iterations must be greater than zero")
    args.function(args)


if __name__ == "__main__":
    main()
