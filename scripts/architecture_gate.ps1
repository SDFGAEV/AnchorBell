$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..')
$strategy = Get-ChildItem (Join-Path $root 'engine\src\strategy') -Filter '*.rs' -Recurse
$forbidden = 'tokio_tungstenite|reqwest|TcpStream|BinanceRestClient|BinanceOrderWebSocket|std::net'
$violations = $strategy | Select-String -Pattern $forbidden
if ($violations) { $violations | ForEach-Object { Write-Error "Decision module exchange I/O: $($_.Path):$($_.LineNumber)" }; exit 1 }
$analytics = Join-Path $root 'engine\src\analytics.rs'
if (Test-Path $analytics) {
  $v = Select-String -Path $analytics -Pattern 'crate::execution|crate::market::live|tokio_tungstenite|reqwest'
  if ($v) { $v | ForEach-Object { Write-Error "Analytics execution coupling: $($_.Path):$($_.LineNumber)" }; exit 1 }
}
$decision = Get-ChildItem (Join-Path $root 'engine\src\strategy'), (Join-Path $root 'engine\src\execution') -Filter '*.rs' -Recurse
$legacy = $decision | Select-String -Pattern 'crate::(evidence|validation_methods)|crate::analytics'
if ($legacy) {
  $legacy | ForEach-Object { Write-Error "Decision/execution layer imports analytics legacy boundary: $($_.Path):$($_.LineNumber)" }
  exit 1
}
$live = Join-Path $root 'engine\src\bin\anchorbell_live.rs'
$simulationFacadeImport = Select-String -Path $live -Pattern '::simulation::|use anchorbell_engine::simulation\s*::'
if ($simulationFacadeImport) {
  $simulationFacadeImport | ForEach-Object { Write-Error "Live runtime imports simulation facade boundary: $($_.Path):$($_.LineNumber)" }
  exit 1
}
Write-Output 'ARCHITECTURE_GATE_PASS'
