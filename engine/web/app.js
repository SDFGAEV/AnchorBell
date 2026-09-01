const $ = (id) => document.getElementById(id);
const activity = $("activity");

function log(message, kind = "") {
  if (activity.classList.contains("empty")) { activity.classList.remove("empty"); activity.textContent = ""; }
  const row = document.createElement("div"); row.className = "log-line " + kind;
  const time = document.createElement("span"); time.className = "time"; time.textContent = new Date().toLocaleTimeString();
  row.append(time, document.createTextNode(message)); activity.prepend(row);
}
function setMessage(message, kind = "") { const target = $("sessionMessage"); target.textContent = message; target.className = "inline-message " + kind; }
function updateStatus(status) {
  $("statusEnvironment").textContent = status.environment;
  $("statusEnvironmentHint").textContent = status.environment === "production" ? "Production 已显式授权" : "默认安全环境";
  $("modeChip").textContent = status.environment.toUpperCase();
  $("statusCredentials").textContent = status.has_credentials ? "已注入" : "未注入";
  $("credentialHint").textContent = status.credential_store_available ? `本机凭证库：${status.saved_credentials ? "已保存" : "未保存"}` : "本机凭证库：当前平台不可用";
  $("statusOrders").textContent = status.allow_order_submission ? "开启" : "关闭";
  $("statusSymbol").textContent = status.symbol; $("statusRegion").textContent = status.region + " · 仅限通过 ADR/ADS 硬过滤的9个标的"; $("allowProduction").checked = status.allow_production;
}
async function api(path, options = {}) {
  const response = await fetch(path, { ...options, headers: { "Content-Type": "application/json", ...(options.headers || {}) } });
  const data = await response.json(); if (!response.ok || data.ok === false) throw new Error(data.message || "请求失败"); return data;
}
async function refreshStatus() { try { updateStatus(await api("/api/status")); } catch (error) { log(error.message, "error"); } }
$("environment").addEventListener("change", (event) => { const production = event.target.value === "production"; $("allowProduction").checked = production; $("allowOrders").checked = false; $("confirmation").disabled = !production; if (!production) $("confirmation").value = ""; });
$("saveSession").addEventListener("click", async () => {
  const button = $("saveSession"); button.disabled = true; setMessage("正在应用…");
  const payload = { environment: $("environment").value, api_key: $("apiKey").value, api_secret: $("apiSecret").value, allow_production: $("allowProduction").checked, allow_order_submission: $("allowOrders").checked, confirmation: $("confirmation").value, symbol: $("symbol").value, proxy: $("proxy").value };
  try { const result = await api("/api/session", { method:"POST", body:JSON.stringify(payload) }); setMessage(result.message,"ok"); log(result.message,"ok"); $("apiKey").value=""; $("apiSecret").value=""; await refreshStatus(); }
  catch (error) { setMessage(error.message,"error"); log(error.message,"error"); } finally { button.disabled=false; }
});

async function credentialRequest() {
  return {
    environment: $("environment").value,
    api_key: $("apiKey").value,
    api_secret: $("apiSecret").value
  };
}
$("saveCredentials").addEventListener("click", async () => {
  const button = $("saveCredentials"); button.disabled = true; setMessage("正在保存到本机凭证库…");
  try {
    const result = await api("/api/credentials/save", { method:"POST", body:JSON.stringify(await credentialRequest()) });
    setMessage(result.message,"ok"); log(result.message,"ok"); $("apiKey").value=""; $("apiSecret").value=""; await refreshStatus();
  } catch (error) { setMessage(error.message,"error"); log(error.message,"error"); } finally { button.disabled=false; }
});
$("deleteCredentials").addEventListener("click", async () => {
  const button = $("deleteCredentials"); button.disabled = true; setMessage("正在删除本机保存凭证…");
  try {
    const result = await api("/api/credentials/delete", { method:"POST", body:JSON.stringify(await credentialRequest()) });
    setMessage(result.message,"ok"); log(result.message,"ok"); await refreshStatus();
  } catch (error) { setMessage(error.message,"error"); log(error.message,"error"); } finally { button.disabled=false; }
});
$("clearSession").addEventListener("click", async () => {
  try { const result = await api("/api/session/clear", { method:"POST", body:"{}" }); $("apiKey").value=""; $("apiSecret").value=""; $("confirmation").value=""; $("allowOrders").checked=false; $("allowProduction").checked=false; setMessage(result.message,"ok"); log(result.message,"ok"); await refreshStatus(); }
  catch (error) { log(error.message,"error"); }
});
document.querySelectorAll(".check-card").forEach((button) => {
  button.addEventListener("click", async () => {
    button.disabled=true; const label=button.querySelector("strong").textContent; log("开始："+label);
    try { const result=await api(button.dataset.endpoint,{method:"POST",body:"{}"}); const details=Object.entries(result).filter(([key])=>!["ok","message"].includes(key)).map(([key,value])=>key+"="+value).join(" · "); log((result.message||"完成")+(details?"（"+details+"）":""),"ok"); }
    catch (error) { log(label+"失败："+error.message,"error"); } finally { button.disabled=false; }
  });
});
$("clearLog").addEventListener("click", () => { activity.className="activity empty"; activity.textContent="还没有操作记录"; });
$("confirmation").disabled=true; refreshStatus();
