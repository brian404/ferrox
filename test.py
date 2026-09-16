#!/usr/bin/env python3
"""
ferrox-compatible demo backend
Serves the landing page + simple API endpoints.
"""

from aiohttp import web
import json

# ---------- The landing page (same professional version) ----------
HTML = r"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>ferrox</title>
  <meta name="description" content="ferrox — a lightweight HTTP server written in Rust">
  <link rel="icon" href="https://i.ibb.co/3yWbP8yH/IMG-20260821-172541-698.jpg" type="image/jpeg">
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:wght@400;500;600&family=IBM+Plex+Sans:wght@400;500;600&display=swap" rel="stylesheet">
  <style>
    :root {
      --base: #14181c; --panel: #1b2126; --panel-2: #20272d;
      --iron: #3a4048; --iron-dim: #2a3036;
      --rust: #b5482f; --ember: #e8794f;
      --text: #eef0f0; --text-dim: #9aa3a8; --text-faint: #5d666c;
      --mono: 'IBM Plex Mono', ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      --sans: 'IBM Plex Sans', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0; background: var(--base);
      background-image:
        radial-gradient(ellipse 900px 500px at 15% -10%, rgba(181,72,47,0.10), transparent 60%),
        radial-gradient(ellipse 700px 400px at 100% 20%, rgba(181,72,47,0.06), transparent 55%);
      color: var(--text); font-family: var(--sans); line-height: 1.6; min-height: 100vh;
    }
    .wrap { max-width: 760px; margin: 0 auto; padding: 4rem 1.5rem 5rem; }
    a { color: var(--ember); }
    header { display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 1rem; margin-bottom: 3rem; }
    .brand { display: flex; align-items: center; gap: 0.7rem; }
    .brand img { width: 28px; height: 28px; display: block; }
    .wordmark { font-family: var(--mono); font-size: 1.15rem; font-weight: 600; letter-spacing: 0.02em; }
    .wordmark span { color: var(--rust); }
    .status-pill {
      display: inline-flex; align-items: center; gap: 0.55rem;
      font-family: var(--mono); font-size: 0.8rem; font-weight: 500; letter-spacing: 0.06em;
      color: #8fdba0; background: rgba(70,180,100,0.08);
      border: 1px solid rgba(70,180,100,0.25); padding: 0.4rem 0.85rem 0.4rem 0.7rem; border-radius: 999px;
    }
    .dot { width: 7px; height: 7px; border-radius: 50%; background: #4fd876;
           box-shadow: 0 0 0 0 rgba(79,216,118,0.6); animation: pulse 2.2s ease-out infinite; }
    @keyframes pulse {
      0%   { box-shadow: 0 0 0 0 rgba(79,216,118,0.55); }
      70%  { box-shadow: 0 0 0 8px rgba(79,216,118,0); }
      100% { box-shadow: 0 0 0 0 rgba(79,216,118,0); }
    }
    .hero h1 { font-family: var(--mono); font-size: clamp(1.75rem,4.5vw,2.35rem); font-weight: 600; margin: 0 0 0.7rem; }
    .hero p { color: var(--text-dim); font-size: 1.05rem; max-width: 48ch; margin: 0 0 1.75rem; }
    .actions { display: flex; gap: 0.75rem; flex-wrap: wrap; }
    .btn { font-family: var(--sans); font-size: 0.92rem; font-weight: 500; text-decoration: none;
           padding: 0.65rem 1.15rem; border-radius: 6px; border: 1px solid transparent; }
    .btn-primary { background: var(--rust); color: #fff; }
    .btn-primary:hover { background: var(--ember); }
    .btn-secondary { background: transparent; color: var(--text); border-color: var(--iron); }
    .btn-secondary:hover { border-color: var(--text-dim); }
    .boot { margin: 3rem 0 3.5rem; background: var(--panel); border: 1px solid var(--iron-dim); border-radius: 10px; overflow: hidden; }
    .boot-titlebar { display: flex; align-items: center; gap: 0.4rem; padding: 0.65rem 1rem; background: var(--panel-2); border-bottom: 1px solid var(--iron-dim); }
    .boot-titlebar .chip { width: 9px; height: 9px; border-radius: 50%; background: var(--iron); }
    .boot-titlebar .label { margin-left: 0.5rem; font-family: var(--mono); font-size: 0.78rem; color: var(--text-faint); }
    .boot-body { padding: 1.1rem 1.3rem 1.3rem; font-family: var(--mono); font-size: 0.88rem; }
    .boot-line { display: flex; gap: 0.6rem; padding: 0.22rem 0; color: var(--text-dim);
                 opacity: 0; transform: translateY(3px); animation: appear 0.4s ease forwards; }
    .boot-line .ok { color: #4fd876; width: 1em; }
    .boot-line .cmd { color: var(--text-faint); }
    .boot-line b { color: var(--text); font-weight: 500; }
    .boot-line:nth-child(1) { animation-delay: 0.05s; }
    .boot-line:nth-child(2) { animation-delay: 0.35s; }
    .boot-line:nth-child(3) { animation-delay: 0.65s; }
    .boot-line:nth-child(4) { animation-delay: 0.95s; }
    .boot-line:nth-child(5) { animation-delay: 1.25s; }
    .boot-line:nth-child(6) { animation-delay: 1.55s; }
    @keyframes appear { to { opacity: 1; transform: translateY(0); } }
    .cursor { display: inline-block; width: 7px; height: 1em; background: var(--ember);
              margin-left: 0.5rem; vertical-align: -0.15em; animation: blink 1s step-end infinite; }
    @keyframes blink { 50% { opacity: 0; } }
    @media (prefers-reduced-motion: reduce) {
      .boot-line { opacity: 1; transform: none; animation: none; }
      .dot, .cursor { animation: none; }
    }
    section.routes { margin: 3.5rem 0 0; }
    .eyebrow { font-family: var(--mono); font-size: 0.78rem; letter-spacing: 0.1em; color: var(--text-faint); text-transform: uppercase; margin: 0 0 0.9rem; }
    table.routes-table { width: 100%; border-collapse: collapse; font-size: 0.94rem; }
    .routes-table tr { border-bottom: 1px solid var(--iron-dim); }
    .routes-table tr:last-child { border-bottom: none; }
    .routes-table td { padding: 0.85rem 0.5rem; vertical-align: top; }
    .method { font-family: var(--mono); font-size: 0.8rem; font-weight: 600; color: var(--rust); white-space: nowrap; padding-right: 1rem; }
    .path { font-family: var(--mono); color: var(--text); white-space: nowrap; padding-right: 1rem; }
    .desc { color: var(--text-dim); }
    footer { margin-top: 4.5rem; padding-top: 1.75rem; border-top: 1px solid var(--iron-dim); font-family: var(--mono); font-size: 0.8rem; color: var(--text-faint); }
    @media (max-width: 520px) { .wrap { padding: 2.75rem 1.25rem 4rem; } .routes-table .desc { display: none; } }
  </style>
</head>
<body>
  <div class="wrap">
    <header>
      <div class="brand">
        <img src="https://i.ibb.co/3yWbP8yH/IMG-20260821-172541-698.jpg" alt="ferrox">
        <div class="wordmark">fer<span>rox</span></div>
      </div>
      <div class="status-pill"><span class="dot"></span>RUNNING</div>
    </header>

    <div class="hero">
      <h1>Lightweight HTTP server</h1>
      <p>ferrox is a single-binary HTTP server written in Rust. If you can see this page, the process is running and accepting connections.</p>
      <div class="actions">
        <a class="btn btn-primary" href="#routes">Documentation</a>
        <a class="btn btn-secondary" href="/health">Health check</a>
      </div>
    </div>

    <div class="boot">
      <div class="boot-titlebar">
        <span class="chip"></span><span class="chip"></span><span class="chip"></span>
        <span class="label">cargo run</span>
      </div>
      <div class="boot-body">
        <div class="boot-line"><span class="cmd">$</span> ./ferrox</div>
        <div class="boot-line"><span class="ok">✓</span> listening on <b>0.0.0.0:3000</b></div>
        <div class="boot-line"><span class="ok">✓</span> static root mounted at <b>./public</b></div>
        <div class="boot-line"><span class="ok">✓</span> proxy configured at <b>/api</b></div>
        <div class="boot-line"><span class="ok">✓</span> path traversal guard active</div>
        <div class="boot-line"><b>ferrox is ready</b><span class="cursor"></span></div>
      </div>
    </div>

    <section class="routes" id="routes">
      <p class="eyebrow">Routes</p>
      <table class="routes-table">
        <tbody>
          <tr><td class="method">GET</td><td class="path">/</td><td class="desc">This page</td></tr>
          <tr><td class="method">GET</td><td class="path">/health</td><td class="desc">Health check — returns 200 when the server is healthy</td></tr>
          <tr><td class="method">GET</td><td class="path">/hello</td><td class="desc">Plain-text response for smoke tests</td></tr>
          <tr><td class="method">GET</td><td class="path">/api/test</td><td class="desc">Simple JSON API endpoint</td></tr>
          <tr><td class="method">GET</td><td class="path">/api/*</td><td class="desc">API namespace</td></tr>
        </tbody>
      </table>
    </section>

    <footer>ferrox · single-binary Rust HTTP server</footer>
  </div>
</body>
</html>
"""

# ---------- Handlers ----------

async def index(request):
    return web.Response(text=HTML, content_type="text/html")

async def health(request):
    return web.Response(text="OK", content_type="text/plain")

async def hello(request):
    return web.Response(text="Hello from ferrox backend\n", content_type="text/plain")

async def api_test(request):
    return web.json_response({"message": "backend alive"})

async def api_echo(request):
    data = await request.json() if request.body_exists else {}
    return web.json_response({
        "method": request.method,
        "path": str(request.rel_url),
        "received": data
    })

# ---------- App ----------

app = web.Application()
app.router.add_get("/", index)
app.router.add_get("/health", health)
app.router.add_get("/hello", hello)
app.router.add_get("/api/test", api_test)
app.router.add_route("*", "/api/echo", api_echo)   # accepts any method

if __name__ == "__main__":
    print("Starting combined server on http://0.0.0.0:8000")
    web.run_app(app, host="0.0.0.0", port=8000)
