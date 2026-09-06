use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use worker::*;

#[derive(Serialize, Deserialize)]
struct TokenRecord {
    name: String,
    created: u64,
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let router = Router::new();

    router
        .get_async("/", |_req, _ctx| async move {
            Response::ok(
                "let-kagi-read-private-repos\n\
                 GET /{owner}/{repo}/{path}?token=...\n\
                 GET /dashboard?key=...",
            )
        })
        .get_async("/dashboard", dashboard_page)
        .post_async("/dashboard/create", create_token)
        .get_async("/:owner/:repo", |req, ctx| async move {
            fetch_github(req, ctx, String::new()).await
        })
        .get_async("/:owner/:repo/*path", |req, ctx| async move {
            let path = ctx.param("path").cloned().unwrap_or_default();
            fetch_github(req, ctx, path).await
        })
        .run(req, env)
        .await
}

// --- dashboard --------------------------------------------------------

fn render_dashboard(key: &str, extra_block: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Kagi: Read my Private Repos</title>
<script src="https://cdn.tailwindcss.com"></script>
<style>
  @keyframes slideDown {{
    0% {{ opacity: 0; transform: translateY(-10px); }}
    100% {{ opacity: 1; transform: translateY(0); }}
  }}
  .animate-slide-down {{
    animation: slideDown 0.4s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  }}
</style>
</head>
<body class="min-h-screen bg-black text-zinc-300 flex items-center justify-center font-mono p-4">
  <div class="w-full max-w-md border border-zinc-800 rounded-lg bg-zinc-950 p-8
              shadow-2xl shadow-red-950/30">
    <h1 class="text-xl font-semibold text-zinc-100 tracking-tight mb-1">
      Let Kagi AI Read my Private Repos
    </h1>
    <p class="text-sm text-zinc-500 mb-6">
      forge a token, hand it off, forget it exists
    </p>

    <form method="POST" action="/dashboard/create?key={key}" class="space-y-4">
      <div>
        <label class="block text-xs uppercase tracking-wide text-zinc-500 mb-1">
          Name for this token
        </label>
        <input
          type="text"
          name="name"
          required
          placeholder="kagi-strikece-thread"
          class="w-full rounded-md bg-zinc-900 border border-zinc-800 px-3 py-2
                 text-zinc-100 placeholder-zinc-600
                 focus:outline-none focus:ring-1 focus:ring-red-900 focus:border-red-900"
        >
      </div>
      <button
        type="submit"
        class="w-full rounded-md bg-red-950 hover:bg-red-900 text-zinc-100
               py-2 transition-colors border border-red-900/50"
      >
        Create
      </button>
    </form>

    {extra_block}
  </div>
</body>
</html>"#,
        key = key,
        extra_block = extra_block
    )
}

fn error_block(msg: &str) -> String {
    format!(
        r#"<div class="mt-8 pt-6 border-t border-zinc-800 animate-slide-down">
      <div class="bg-red-950/30 border border-red-900/50 p-4 rounded-md">
        <h2 class="text-sm font-semibold text-red-500 mb-1">Forging Failed</h2>
        <p class="text-xs text-red-400/80">{msg}</p>
      </div>
    </div>"#,
        msg = msg
    )
}

async fn dashboard_page(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let url = req.url()?;
    let key = query_param(&url, "key");
    let expected = ctx.secret("DASHBOARD_KEY")?.to_string();

    if key.as_deref() != Some(expected.as_str()) {
        return Response::error("Forbidden", 403);
    }

    html(render_dashboard(&expected, ""))
}

async fn create_token(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let url = req.url()?;
    let key = query_param(&url, "key");
    let expected = ctx.secret("DASHBOARD_KEY")?.to_string();

    if key.as_deref() != Some(expected.as_str()) {
        return Response::error("Forbidden", 403);
    }

    let form = match req.form_data().await {
        Ok(f) => f,
        Err(_) => return html(render_dashboard(&expected, &error_block("Invalid form data payload"))),
    };

    let name = match form.get("name") {
        Some(FormEntry::Field(value)) if !value.trim().is_empty() => value,
        _ => return html(render_dashboard(&expected, &error_block("Missing or empty name field"))),
    };

    let token = generate_token();
    let hash = sha1_hex(&token);
    let record = TokenRecord { name: name.clone(), created: Date::now().as_millis() };

    let kv = match ctx.kv("TOKENS") {
        Ok(k) => k,
        Err(_) => return html(render_dashboard(&expected, &error_block("KV binding 'TOKENS' not found"))),
    };

    let put_req = match kv.put(&hash, serde_json::to_string(&record)?) {
        Ok(r) => r,
        Err(_) => return html(render_dashboard(&expected, &error_block("Failed to prepare KV data"))),
    };

    if let Err(e) = put_req.execute().await {
        return html(render_dashboard(&expected, &error_block(&format!("Failed to save token to KV: {}", e))));
    }

    let host = url.host_str().unwrap_or("let-kagi-read-my-private-repos.zbee.codes");
    let example = format!("https://{host}/OWNER/REPO/path/to/file?token={token}");

    let success_html = format!(
        r#"<div class="mt-8 pt-6 border-t border-zinc-800 animate-slide-down">
      <h2 class="text-sm font-semibold text-emerald-500 mb-2">
        Token created for <span class="text-emerald-400">"{name}"</span>
      </h2>
      <p class="text-xs text-zinc-500 mb-4">
        Copy this now, it will not be shown again.
      </p>
      <div class="relative group">
        <div class="absolute -inset-0.5 bg-red-900/30 blur opacity-75 group-hover:opacity-100 transition duration-200 rounded"></div>
        <div class="relative bg-black p-3 rounded border border-zinc-800 font-mono text-xs break-all text-zinc-300 select-all">
          {token}
        </div>
      </div>
      <p class="text-xs text-zinc-500 mt-5 mb-1 uppercase tracking-wider">Example URL</p>
      <div class="bg-black p-3 rounded border border-zinc-800 font-mono text-[10px] break-all text-zinc-500 select-all">
        {example}
      </div>
    </div>"#,
        name = name,
        token = token,
        example = example
    );

    html(render_dashboard(&expected, &success_html))
}

// --- github read side ---------------------------------------------------

async fn fetch_github(req: Request, ctx: RouteContext<()>, path: String) -> Result<Response> {
    let url = req.url()?;
    let Some(token) = query_param(&url, "token") else {
        return Response::error("Missing ?token=", 401);
    };

    let kv = ctx.kv("TOKENS")?;
    let hash = sha1_hex(&token);
    if kv.get(&hash).json::<TokenRecord>().await?.is_none() {
        return Response::error("Invalid token", 403);
    }

    let owner = ctx.param("owner").unwrap();
    let repo = ctx.param("repo").unwrap();

    let pats_raw = ctx.secret("GITHUB_TOKENS")?.to_string();
    let pats = pats_raw.split(',').map(str::trim).filter(|s| !s.is_empty());

    for pat in pats {
        if let Some(resp) = try_github(owner, repo, &path, pat).await? {
            return Ok(resp);
        }
    }

    Response::error("Not found in any configured account/org", 404)
}

async fn try_github(owner: &str, repo: &str, path: &str, pat: &str) -> Result<Option<Response>> {
    let api_url = format!("https://api.github.com/repos/{owner}/{repo}/contents/{path}");

    let headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {pat}"))?;
    headers.set("User-Agent", "let-kagi-read-privates-worker")?;
    headers.set("Accept", "application/vnd.github+json")?;
    headers.set("X-GitHub-Api-Version", "2022-11-28")?;

    let mut init = RequestInit::new();
    init.with_method(Method::Get).with_headers(headers);

    let request = Request::new_with_init(&api_url, &init)?;
    let mut gh_response = Fetch::Request(request).send().await?;

    match gh_response.status_code() {
        200 => Ok(Some(format_github_body(&mut gh_response, owner, repo, path).await?)),
        _ => Ok(None), // this PAT can't see it, try the next one
    }
}

async fn format_github_body(
    gh_response: &mut Response,
    owner: &str,
    repo: &str,
    path: &str,
) -> Result<Response> {
    let body: serde_json::Value = gh_response.json().await?;

    if let Some(items) = body.as_array() {
        let mut listing = format!("Directory listing for {owner}/{repo}/{path}\n\n");
        for item in items {
            let name = item["name"].as_str().unwrap_or("?");
            let kind = item["type"].as_str().unwrap_or("?");
            let item_path = item["path"].as_str().unwrap_or("?");
            listing.push_str(&format!("[{kind}] {name}  ->  /{owner}/{repo}/{item_path}\n"));
        }
        return plain_text(listing);
    }

    if body["type"] == "file" {
        let encoded: String = body["content"]
            .as_str()
            .unwrap_or("")
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let decoded = STANDARD.decode(encoded).map_err(|e| Error::from(e.to_string()))?;
        return plain_text(String::from_utf8_lossy(&decoded).to_string());
    }

    Response::error("Unsupported content type", 415)
}

// --- helpers --------------------------------------------------------

fn query_param(url: &Url, key: &str) -> Option<String> {
    url.query_pairs().find(|(k, _)| k == key).map(|(_, v)| v.into_owned())
}

fn sha1_hex(input: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

fn generate_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn html(body: String) -> Result<Response> {
    let mut resp = Response::ok(body)?;
    resp.headers_mut().set("Content-Type", "text/html; charset=utf-8")?;
    Ok(resp)
}

fn plain_text(body: String) -> Result<Response> {
    let mut resp = Response::ok(body)?;
    resp.headers_mut().set("Content-Type", "text/plain; charset=utf-8")?;
    Ok(resp)
}
