#!/usr/bin/env python3
"""A local page that shows where the project stands, read from origin/main (specs/modules/tracking.md).

Usage: scripts/dashboard.py [--port 7320] [--no-open]
"""
import json
import plistlib
import re
import subprocess
import sys
import threading
import time
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REPO = "https://github.com/Aottens/CMDMX"
REFRESH_SECONDS = 20
APPS = ("/Applications/CMDMX.app", "/Applications/ColourmotionDMX.app")  # brand: old-name, installs from before the rename


def run(*cmd):
    return subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, check=True, timeout=60).stdout


def on_main(path):
    return run("git", "show", f"origin/main:{path}")


def parse_backlog(text):
    """Every BACKLOG table row, with the heading it sits under."""
    tasks, section = [], ""
    for line in text.splitlines():
        heading = re.match(r"#{2,3} (.+)", line)
        if heading:
            section = heading.group(1).split(" (")[0]
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if line.startswith("| ") and cells[0] not in ("Id", "---") and len(cells) >= 3:
            tasks.append({
                "id": cells[0], "task": cells[1], "spec": cells[2] if len(cells) >= 5 else "",
                "size": cells[-2] if len(cells) >= 4 else "", "status": cells[-1].split(":")[0].split(" ")[0],
                "detail": cells[-1], "section": section,
            })
    return tasks


def parse_sections(text):
    """The `## ` sections of STATE.md as {title: body}."""
    return {m[0]: m[1].strip() for m in re.findall(r"^## ([^\n]+)\n(.*?)(?=^## |\Z)", text, re.M | re.S)}


def checks(pr):
    results = [c.get("conclusion") or c.get("state") or c.get("status") for c in pr.pop("statusCheckRollup", [])]
    if any(r in ("FAILURE", "ERROR", "TIMED_OUT", "CANCELLED") for r in results):
        return "failing"
    return "passing" if results and all(r in ("SUCCESS", "NEUTRAL", "SKIPPED") for r in results) else "pending"


def open_prs():
    prs = json.loads(run("gh", "pr", "list", "--state", "open", "--json", "number,title,url,isDraft,statusCheckRollup"))
    return [{**pr, "checks": checks(pr)} for pr in prs]


def merged_prs():
    return json.loads(run("gh", "pr", "list", "--state", "merged", "--limit", "6", "--json", "number,title,url,mergedAt"))


def releases():
    found = json.loads(run("gh", "release", "list", "--limit", "5", "--json", "tagName,publishedAt,isLatest"))
    return [{**r, "url": f"{REPO}/releases/tag/{r['tagName']}"} for r in found]


def workflows():
    """The newest run of each workflow on main."""
    runs = json.loads(run("gh", "run", "list", "--branch", "main", "--limit", "12", "--json",
                          "workflowName,conclusion,status,url"))
    return list({r["workflowName"]: r for r in reversed(runs)}.values())


def head():
    sha, subject, date = run("git", "log", "-1", "--format=%h%x00%s%x00%cI", "origin/main").strip().split("\0")
    return {"sha": sha, "subject": subject, "date": date}


def installed():
    for app in APPS:
        plist = Path(app) / "Contents/Info.plist"
        if plist.exists():
            with plist.open("rb") as f:
                return {"app": Path(app).name, "version": plistlib.load(f).get("CFBundleShortVersionString", "?")}
    return {"app": "", "version": ""}


def update(snapshot, name, read, now=None):
    """Replace one part of the snapshot; on failure keep the last good data and say so (TRK-08)."""
    now = time.time() if now is None else now
    try:
        snapshot[name] = read()
        snapshot["sources"][name] = {"at": now, "error": ""}
    except Exception as error:  # a dashboard outlives anything git or gh can do
        last = snapshot["sources"].get(name, {}).get("at", 0)
        text = getattr(error, "stderr", "") or str(error)
        snapshot["sources"][name] = {"at": last, "error": text.strip()[:200]}


snapshot = {"sources": {}}
lock = threading.Lock()


def refresh():
    with lock:
        next_part = {**snapshot, "sources": dict(snapshot["sources"])}
    update(next_part, "fetch", lambda: run("git", "fetch", "-q", "origin", "main") and None)
    update(next_part, "head", head)
    update(next_part, "state", lambda: parse_sections(on_main("STATE.md")))
    update(next_part, "tasks", lambda: parse_backlog(on_main("BACKLOG.md")))
    update(next_part, "prs", open_prs)
    update(next_part, "merged", merged_prs)
    update(next_part, "releases", releases)
    update(next_part, "workflows", workflows)
    update(next_part, "installed", installed)
    next_part["refreshed"] = time.time()
    with lock:
        snapshot.clear()
        snapshot.update(next_part)


def refresh_forever():
    while True:
        refresh()
        time.sleep(REFRESH_SECONDS)


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/state.json":
            with lock:
                body, kind = json.dumps(snapshot).encode(), "application/json"
        elif self.path == "/":
            body, kind = PAGE.encode(), "text/html; charset=utf-8"
        else:
            self.send_error(404)
            return
        self.send_response(200)
        self.send_header("Content-Type", kind)
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


PAGE = r"""<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>CMDMX State</title>
<style>
:root { color-scheme: light dark; --bg:#f5f4f0; --panel:#fff; --line:#dddbd3; --text:#22211d; --dim:#6d6a60;
  --accent:#a8480c; --good:#2d7a3e; --wait:#a8480c; --bad:#bb2d2d; --open:#3a62a8; --bar:#e6e4dc; --warn-bg:#fbeee3; }
@media (prefers-color-scheme: dark) { :root { --bg:#16161a; --panel:#1f1f24; --line:#33333a; --text:#ecebe6; --dim:#9a988f;
  --accent:#f0a060; --good:#62c47a; --wait:#f0a060; --bad:#ef7070; --open:#83a6e6; --bar:#2c2c32; --warn-bg:#35261a; } }
* { box-sizing:border-box; }
body { margin:0; background:var(--bg); color:var(--text); font:14px/1.45 -apple-system,system-ui,sans-serif; }
header { position:sticky; top:0; z-index:2; display:flex; flex-wrap:wrap; gap:6px 16px; align-items:center;
  padding:10px 20px; background:var(--panel); border-bottom:1px solid var(--line); }
h1 { font-size:15px; margin:0; } h2 { font-size:12px; letter-spacing:.05em; text-transform:uppercase; color:var(--dim); margin:0 0 8px; }
main { display:grid; gap:14px; padding:14px 20px 40px; grid-template-columns:minmax(0,3fr) minmax(0,2fr); max-width:1500px; margin:auto; }
section { background:var(--panel); border:1px solid var(--line); border-radius:8px; padding:12px 14px; min-width:0; }
.wide { grid-column:1/-1; } .owner { border-color:var(--accent); border-width:2px; } .owner h2 { color:var(--accent); }
@media (max-width:860px) { main { grid-template-columns:minmax(0,1fr); padding:12px 16px 32px; } header { padding:10px 16px; } }
.meta { color:var(--dim); font-size:12.5px; } .push { margin-left:auto; }
.dot { display:inline-block; width:8px; height:8px; border-radius:50%; margin-right:5px; background:var(--dim); }
.success,.passing,.done { --c:var(--good); } .failure,.failing { --c:var(--bad); } .gate3 { --c:var(--wait); }
.blocked { --c:var(--bad); } .todo,.doing,.pending { --c:var(--open); }
.dot.success,.dot.passing,.dot.failure,.dot.failing,.dot.pending { background:var(--c); }
.md p { margin:0 0 8px; } .md ul,.md ol { margin:0 0 8px; padding-left:20px; } .md li { margin-bottom:4px; }
code { font:12px ui-monospace,Menlo,monospace; overflow-wrap:anywhere; }
button.id { all:unset; cursor:pointer; color:var(--accent); font-weight:600; white-space:nowrap; }
button.id:hover,button.id:focus-visible { text-decoration:underline; }
.warn { grid-column:1/-1; background:var(--warn-bg); border:1px solid var(--bad); border-radius:8px; padding:8px 14px; }
.warn:empty { display:none; }
.bars { display:grid; gap:2px; grid-template-columns:repeat(auto-fill,minmax(310px,1fr)); column-gap:24px; }
.bar { all:unset; cursor:pointer; display:grid; grid-template-columns:minmax(0,1fr) 54px; gap:0 8px; padding:4px 6px; border-radius:5px; }
.bar:hover,.bar:focus-visible,.bar.on { background:var(--bar); }
.bar span { white-space:nowrap; overflow:hidden; text-overflow:ellipsis; } .bar b { text-align:right; font-weight:500; font-variant-numeric:tabular-nums; color:var(--dim); }
.track { grid-column:1/-1; height:5px; display:flex; border-radius:3px; overflow:hidden; background:var(--bar); margin-top:2px; }
.track i { display:block; background:var(--c); }
.tools { display:flex; flex-wrap:wrap; gap:6px; align-items:center; margin-bottom:10px; }
input[type=search] { flex:1 1 180px; min-width:0; padding:6px 9px; border:1px solid var(--line); border-radius:6px; background:var(--bg); color:inherit; font:inherit; }
.chip { all:unset; cursor:pointer; padding:3px 10px; border:1px solid var(--line); border-radius:12px; font-size:12.5px; }
.chip.on { background:var(--text); color:var(--panel); border-color:var(--text); }
.chip:focus-visible { outline:2px solid var(--accent); }
.tablewrap { overflow-x:auto; } table { border-collapse:collapse; width:100%; }
th { text-align:left; font-size:12px; color:var(--dim); font-weight:500; padding:4px 8px; }
td { padding:6px 8px; border-top:1px solid var(--line); vertical-align:top; }
tbody tr { cursor:pointer; } tbody tr:hover { background:var(--bar); } tbody tr:focus-visible { outline:2px solid var(--accent); outline-offset:-2px; }
td.task { width:100%; min-width:260px; } tr:not(.open) td.task div { display:-webkit-box; -webkit-line-clamp:1; -webkit-box-orient:vertical; overflow:hidden; }
td.nowrap { white-space:nowrap; } .small { color:var(--dim); font-size:12.5px; margin-top:4px; }
.badge { color:var(--c); font-weight:600; font-size:12.5px; }
.list { list-style:none; margin:0; padding:0; } .list li { padding:5px 0; border-top:1px solid var(--line); } .list li:first-child { border-top:0; }
a { color:inherit; } .empty { color:var(--dim); }
</style></head><body>
<header><h1>CMDMX State</h1><span class="meta" id="head"></span><span class="meta" id="workflows"></span>
<span class="meta push" id="age" aria-live="polite"></span></header>
<main>
<div class="warn" id="warn" role="alert"></div>
<section class="wide owner"><h2>Waiting on you</h2><div class="md" id="waiting"></div></section>
<section><h2>Now</h2><div class="md" id="now"></div><h2 style="margin-top:14px">Next</h2><div class="md" id="next"></div></section>
<section><h2>Open pull requests</h2><ul class="list" id="prs"></ul>
<h2 style="margin-top:14px">Releases and this Mac</h2><ul class="list" id="releases"></ul></section>
<section class="wide"><h2>Progress per backlog section</h2><div class="bars" id="bars"></div></section>
<section class="wide"><h2>Tasks</h2><div class="tools"><input type="search" id="q" placeholder="Search id or text" aria-label="Search tasks">
<span id="chips"></span><button class="chip" id="clear" hidden></button></div>
<div class="tablewrap"><table><thead><tr><th>Id</th><th>Task</th><th>Size</th><th>Status</th><th>Section</th></tr></thead>
<tbody id="rows"></tbody></table></div><div class="small" id="count"></div></section>
<section><h2>Blocked</h2><div class="md" id="blocked"></div><h2 style="margin-top:14px">To decide</h2><div class="md" id="decide"></div></section>
<section><h2>Recently merged</h2><ul class="list" id="merged"></ul><h2 style="margin-top:14px">Good to know</h2><div class="md" id="know"></div></section>
</main>
<script>
const $ = id => document.getElementById(id);
const esc = s => String(s ?? "").replace(/[&<>"']/g, c => ({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#39;"}[c]));
const FILTERS = {open:"Open", gate3:"Gate 3", blocked:"Blocked", todo:"To do", done:"Done", all:"All"};
const view = {status:"open", section:"", q:"", open:new Set()};
let data = {}, ids = new Set(), shown = "";

function inline(text) {
  return esc(text).replace(/`([^`]+)`/g, "<code>$1</code>").replace(/\*\*([^*]+)\*\*/g, "<b>$1</b>")
    .replace(/\*([^*]+)\*/g, "<i>$1</i>").replace(/\[([^\]]+)\]\((https?:[^)]+)\)/g, '<a href="$2" target="_blank" rel="noopener">$1</a>')
    .replace(/\b[A-Z][A-Z0-9]*-[0-9]+\b/g, m => ids.has(m) ? `<button class="id" data-id="${m}">${m}</button>` : m);
}
function markdown(text) {
  const out = []; let list = null;
  const close = () => { if (list) { out.push(`</${list}>`); list = null; } };
  const blocks = [];
  for (const line of (text || "").split("\n")) {
    const item = line.match(/^(-|\d+\.) (.*)/);
    if (item) blocks.push({tag:item[1] === "-" ? "ul" : "ol", text:item[2]});
    else if (!line.trim()) blocks.push(null);
    else if (line.startsWith(" ") && blocks.length && blocks[blocks.length-1]) blocks[blocks.length-1].text += " " + line.trim();
    else if (blocks.length && blocks[blocks.length-1] && !blocks[blocks.length-1].tag) blocks[blocks.length-1].text += " " + line.trim();
    else blocks.push({text:line});
  }
  for (const b of blocks) {
    if (!b) { close(); continue; }
    if (!b.tag) { close(); out.push(`<p>${inline(b.text)}</p>`); continue; }
    if (list !== b.tag) { close(); out.push(`<${b.tag}>`); list = b.tag; }
    out.push(`<li>${inline(b.text)}</li>`);
  }
  close();
  return out.join("") || '<span class="empty">Nothing.</span>';
}
const ago = at => { if (!at) return "never"; const s = Math.max(0, Date.now()/1000 - at);
  return s < 90 ? `${Math.round(s)} s ago` : s < 5400 ? `${Math.round(s/60)} min ago` : s < 129600 ? `${Math.round(s/3600)} h ago` : `${Math.round(s/86400)} days ago`; };
const date = iso => esc((iso || "").slice(0, 10));

function matches(t) {
  const q = view.q.toLowerCase();
  const byStatus = view.status === "all" || (view.status === "open" ? t.status !== "done" : t.status === view.status);
  return byStatus && (!view.section || t.section === view.section) && (!q || (t.id + " " + t.task + " " + t.spec).toLowerCase().includes(q));
}
function renderTasks() {
  const tasks = data.tasks || [];
  $("chips").innerHTML = Object.entries(FILTERS).map(([k, label]) =>
    `<button class="chip ${view.status === k ? "on" : ""}" data-status="${k}" aria-pressed="${view.status === k}">${label}</button> `).join("");
  $("clear").hidden = !view.section; $("clear").textContent = view.section ? `${view.section} ✕` : "";
  const rows = tasks.filter(matches);
  $("rows").innerHTML = rows.map(t => { const open = view.open.has(t.id); return `<tr class="${open ? "open" : ""}" data-row="${esc(t.id)}" tabindex="0" aria-expanded="${open}">
    <td class="nowrap"><b>${esc(t.id)}</b></td><td class="task"><div>${inline(t.task)}</div>${open ? `<div class="small">Spec: ${inline(t.spec) || "–"} · ${inline(t.detail)}</div>` : ""}</td>
    <td>${esc(t.size)}</td><td class="nowrap"><span class="badge ${esc(t.status)}">${esc(t.status)}</span></td><td class="nowrap meta">${esc(t.section)}</td></tr>`; }).join("")
    || '<tr><td colspan="5" class="empty">No task matches.</td></tr>';
  $("count").textContent = `${rows.length} of ${tasks.length} tasks`;
}
function renderBars() {
  const by = new Map();
  for (const t of data.tasks || []) { if (!by.has(t.section)) by.set(t.section, []); by.get(t.section).push(t); }
  $("bars").innerHTML = [...by].map(([name, ts]) => {
    const n = s => ts.filter(t => t.status === s).length, done = n("done"), gate = n("gate3"), blocked = n("blocked"), pct = x => 100 * x / ts.length;
    return `<button class="bar ${view.section === name ? "on" : ""}" data-section="${esc(name)}" title="${esc(name)}"><span>${esc(name)}</span><b>${done}/${ts.length}</b>
      <div class="track"><i class="done" style="width:${pct(done)}%"></i><i class="gate3" style="width:${pct(gate)}%"></i><i class="blocked" style="width:${pct(blocked)}%"></i></div></button>`; }).join("");
}
function render() {
  const s = data.state || {};
  ids = new Set((data.tasks || []).map(t => t.id));
  $("waiting").innerHTML = markdown(s["Waiting on the owner"]);
  $("now").innerHTML = markdown(s["Now"]); $("next").innerHTML = markdown(s["Next"]);
  $("blocked").innerHTML = markdown(s["Blocked"]); $("decide").innerHTML = markdown(s["Owner to decide"]); $("know").innerHTML = markdown(s["Good to know"]);
  const h = data.head || {};
  $("head").innerHTML = h.sha ? `main <code>${esc(h.sha)}</code> ${esc(h.subject)}` : "";
  $("workflows").innerHTML = (data.workflows || []).map(w => { const state = w.conclusion || "pending";
    return `<a href="${esc(w.url)}" target="_blank" rel="noopener" title="${esc(state)}"><span class="dot ${esc(state)}"></span>${esc(w.workflowName)}</a>`; }).join(" &nbsp; ");
  $("prs").innerHTML = (data.prs || []).map(p => `<li><span class="dot ${esc(p.checks)}" title="checks ${esc(p.checks)}"></span><a href="${esc(p.url)}" target="_blank" rel="noopener">#${p.number} ${esc(p.title)}</a>${p.isDraft ? " (draft)" : ""}<div class="small">checks ${esc(p.checks)}</div></li>`).join("") || '<li class="empty">None: nothing waits for your merge.</li>';
  const inst = data.installed || {}, latest = (data.releases || []).find(r => r.isLatest);
  const mac = !inst.app ? "Not installed on this Mac." : `This Mac: ${esc(inst.app)} ${esc(inst.version)}` + (latest && latest.tagName !== "v" + inst.version ? `, latest is ${esc(latest.tagName)}` : "");
  $("releases").innerHTML = `<li>${mac}</li>` + (data.releases || []).map(r => `<li><a href="${esc(r.url)}" target="_blank" rel="noopener">${esc(r.tagName)}</a> <span class="meta">${date(r.publishedAt)}${r.isLatest ? " · latest" : ""}</span></li>`).join("");
  $("merged").innerHTML = (data.merged || []).map(p => `<li><a href="${esc(p.url)}" target="_blank" rel="noopener">#${p.number} ${esc(p.title)}</a> <span class="meta">${date(p.mergedAt)}</span></li>`).join("") || '<li class="empty">Nothing yet.</li>';
  renderBars(); renderTasks();
}
function renderAge(failed) {
  $("age").textContent = failed ? "Dashboard not reachable: showing the last data" : data.refreshed ? `Checked ${ago(data.refreshed)}` : "First check running…";
  const bad = Object.entries(data.sources || {}).filter(([, v]) => v.error);
  $("warn").innerHTML = failed ? "The dashboard process has stopped; start <code>scripts/dashboard.py</code> again." :
    bad.map(([k, v]) => `<div><b>${esc(k)}</b> failed, data from ${esc(ago(v.at))}: ${esc(v.error)}</div>`).join("");
}
async function poll() {
  try {
    const text = await (await fetch("/state.json", {cache:"no-store"})).text();
    data = JSON.parse(text);
    const content = JSON.stringify({...data, sources:0, refreshed:0});
    if (content !== shown) { shown = content; render(); }
    renderAge(false);
  } catch { renderAge(true); }
}
document.addEventListener("click", e => {
  const el = e.target.closest("button,tr[data-row],a"); if (!el) return;
  if (el.dataset.id) { view.q = el.dataset.id; view.status = "all"; view.section = ""; view.open.add(el.dataset.id); $("q").value = view.q; renderBars(); renderTasks(); $("q").scrollIntoView({block:"center"}); }
  else if (el.dataset.status) { view.status = el.dataset.status; renderTasks(); }
  else if (el.dataset.section !== undefined) { view.section = view.section === el.dataset.section ? "" : el.dataset.section; renderBars(); renderTasks(); }
  else if (el.id === "clear") { view.section = ""; renderBars(); renderTasks(); }
  else if (el.dataset.row) { const id = el.dataset.row; view.open.has(id) ? view.open.delete(id) : view.open.add(id); renderTasks(); }
});
document.addEventListener("keydown", e => { if ((e.key === "Enter" || e.key === " ") && e.target.dataset.row) { e.preventDefault(); e.target.click(); } });
$("q").addEventListener("input", e => { view.q = e.target.value; renderTasks(); });
poll(); setInterval(poll, 3000);
</script></body></html>
"""


def main():
    port = int(sys.argv[sys.argv.index("--port") + 1]) if "--port" in sys.argv else 7320
    try:
        server = ThreadingHTTPServer(("127.0.0.1", port), Handler)
    except OSError as error:
        sys.exit(f"Port {port} is taken ({error}): is the dashboard already running? http://127.0.0.1:{port}/")
    threading.Thread(target=refresh_forever, daemon=True).start()
    url = f"http://127.0.0.1:{port}/"
    print(f"CMDMX state on {url} (Ctrl-C stops it)")
    if "--no-open" not in sys.argv:
        webbrowser.open(url)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
