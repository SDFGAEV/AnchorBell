from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOTS = (ROOT / "engine" / "src", ROOT / "engine" / "web")
MAX_SOURCE_BYTES = 250_000
MAX_LINE_BYTES = 16_384
failures = []

for source_root in SOURCE_ROOTS:
    for path in source_root.rglob("*"):
        if not path.is_file() or path.suffix not in {".rs", ".js", ".html", ".css"}:
            continue
        data = path.read_bytes()
        if len(data) > MAX_SOURCE_BYTES:
            failures.append(f"source budget exceeded: {path} ({len(data)} bytes)")
        for number, line in enumerate(data.splitlines(), 1):
            if len(line) > MAX_LINE_BYTES:
                failures.append(f"line budget exceeded: {path}:{number}")

if failures:
    raise SystemExit("resource gate failed:\n" + "\n".join(failures))
print("RESOURCE_GATE_PASS")
