from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
strategy = ROOT / "engine" / "src" / "strategy"
execution = ROOT / "engine" / "src" / "execution"
# Exchange I/O belongs to the execution adapter. Strategy remains pure and
# communicates through typed intents and snapshots only.
production_roots = [strategy]

forbidden_exchange_io = re.compile(
    r"tokio_tungstenite|reqwest|TcpStream|BinanceRestClient|"
    r"BinanceOrderWebSocket|std::net"
)
for path in [p for root in production_roots for p in root.rglob("*.rs")]:
    text = path.read_text(encoding="utf-8", errors="replace")
    match = forbidden_exchange_io.search(text)
    if match:
        raise SystemExit(f"strategy exchange I/O: {path}:{match.group(0)}")

analytics = ROOT / "engine" / "src" / "analytics.rs"
analytics_text = analytics.read_text(encoding="utf-8")
if re.search(r"crate::execution|crate::market::live|tokio_tungstenite|reqwest", analytics_text):
    raise SystemExit(f"analytics execution coupling: {analytics}")
decision_execution = [
    p for root in (strategy, execution) for p in root.rglob("*.rs")
]
legacy_boundary = re.compile(r"crate::(analytics_evidence|analytics_validation|analytics)")
for path in decision_execution:
    text = path.read_text(encoding="utf-8", errors="replace")
    if legacy_boundary.search(text):
        raise SystemExit(f"decision/execution analytics coupling: {path}")

live = ROOT / "engine" / "src" / "bin" / "anchorbell_live.rs"
live_text = live.read_text(encoding="utf-8")
if re.search(r"::simulation::|use static_anchor_engine::simulation\s*::", live_text):
    raise SystemExit(f"live runtime imports simulation facade: {live}")

web = ROOT / "engine" / "web"
for path in web.rglob("*"):
    if path.is_file():
        text = path.read_text(encoding="utf-8", errors="replace")
        for term in ("paper", "PAPER", "Paper", "market_legacy_exports", "validation_methods"):
            if term in text:
                raise SystemExit(f"forbidden production vocabulary: {path}:{term}")

print("ARCHITECTURE_GATE_PASS")
