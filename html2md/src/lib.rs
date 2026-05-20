use anyhow::{anyhow, Context, Result};
use scraper::{Html, Selector};
use spin_sdk::http::{IntoResponse, Request, Response, Router};
use spin_sdk::http_component;
use std::collections::BTreeMap;

/// A Spin Function that fetches a URL and converts the HTML content to Markdown.
///
/// Usage:
///   GET  /?url=https://example.com
///   POST / with the URL as the plain-text request body
#[http_component]
async fn handle_html2md(req: Request) -> Response {
    let mut router = Router::new();
    router.get_async("/", handle_get);
    router.post_async("/", handle_post);
    router.handle_async(req).await
}

/// Handle GET requests — extract the URL from the `url` query parameter.
async fn handle_get(req: Request, _params: spin_sdk::http::Params) -> Result<impl IntoResponse> {
    let full_url = req
        .header("spin-full-url")
        .and_then(|v| v.as_str())
        .unwrap_or(req.uri());

    let parsed = url::Url::parse(full_url)
        .context("Failed to parse the request URL for query extraction")?;

    let url = parsed
        .query_pairs()
        .find(|(key, _)| key == "url")
        .map(|(_, value)| value.into_owned())
        .ok_or_else(|| {
            anyhow!("Missing required `url` query parameter. Usage: GET /?url=https://example.com")
        })?;

    fetch_and_convert(&url).await
}

/// Handle POST requests. Dispatches on `Content-Type`:
///   * `text/html`     — body is HTML to convert directly (no fetch).
///   * anything else   — body is a plain-text URL to fetch.
async fn handle_post(
    req: Request,
    _params: spin_sdk::http::Params,
) -> Result<impl IntoResponse> {
    let content_type = req
        .header("content-type")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_lowercase();

    let body = req.body();

    if content_type.starts_with("text/html") {
        let html = std::str::from_utf8(body)
            .context("Request body is not valid UTF-8")?;
        if html.trim().is_empty() {
            return Err(anyhow!(
                "POST body is empty. Send HTML as the request body with Content-Type: text/html."
            ));
        }
        return convert_to_markdown(html, None);
    }

    let url = std::str::from_utf8(body)
        .context("Request body is not valid UTF-8")?
        .trim()
        .to_string();

    if url.is_empty() {
        return Err(anyhow!(
            "POST body is empty. Send the target URL as the request body, or send HTML with Content-Type: text/html."
        ));
    }

    fetch_and_convert(&url).await
}

const MAX_REDIRECTS: u8 = 10;

/// Fetch a URL (following redirects) and convert its HTML content to Markdown.
async fn fetch_and_convert(url: &str) -> Result<Response> {
    let mut current_url = url.to_string();

    let resp = 'redirect: {
        for _ in 0..MAX_REDIRECTS {
            let fetch_req = Request::builder()
                .method(spin_sdk::http::Method::Get)
                .uri(&current_url)
                .header("accept", "text/html")
                .header(
                    "user-agent",
                    "AgentMD/0.1 (HTML-to-Markdown converter for AI agents)",
                )
                .build();

            let resp: Response = spin_sdk::http::send(fetch_req).await?;
            let status = *resp.status();

            if (300..400).contains(&status) {
                let location = resp
                    .header("location")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        anyhow!("Received HTTP {status} redirect without a Location header")
                    })?;

                // Resolve relative redirects against the current URL.
                current_url = url::Url::parse(&current_url)
                    .and_then(|base| base.join(location))
                    .map(|u| u.to_string())
                    .unwrap_or_else(|_| location.to_string());

                continue;
            }

            break 'redirect resp;
        }

        return Ok(Response::builder()
            .status(502)
            .header("content-type", "text/plain")
            .body(format!("Too many redirects when fetching {url}"))
            .build());
    };

    let status = *resp.status();
    if !(200..300).contains(&status) {
        return Ok(Response::builder()
            .status(502)
            .header("content-type", "text/plain")
            .body(format!(
                "Upstream returned HTTP {status} when fetching {current_url}"
            ))
            .build());
    }

    // Check the Content-Type to ensure we got HTML.
    let content_type = resp
        .header("content-type")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_lowercase();

    if !content_type.contains("text/html") {
        return Ok(Response::builder()
            .status(400)
            .header("content-type", "text/plain")
            .body(format!(
                "The URL did not return HTML (Content-Type: {content_type})"
            ))
            .build());
    }

    let html = resp.into_body();
    let html_str = String::from_utf8(html).context("Response body is not valid UTF-8")?;

    convert_to_markdown(&html_str, Some(&current_url))
}

/// Convert an HTML string to a Markdown response.
///
/// When `base_url` is `Some`, relative `<a href>` / `<img src>` and metadata
/// URLs (canonical, og:image, twitter:image) are resolved against it. When it
/// is `None`, those values are emitted unchanged.
fn convert_to_markdown(html_str: &str, base_url: Option<&str>) -> Result<Response> {
    let base_url_a = base_url.map(|s| s.to_string());
    let base_url_img = base_url.map(|s| s.to_string());

    let converter = htmd::HtmlToMarkdown::builder()
        .skip_tags(vec!["script", "style", "noscript"])
        // Rewrite <a href> to absolute URLs when a base is available.
        .add_handler(vec!["a"], move |element: htmd::Element| {
            let mut href: Option<String> = None;
            let mut title: Option<String> = None;
            for attr in element.attrs {
                match attr.name.local.as_ref() {
                    "href" => href = Some(attr.value.to_string()),
                    "title" => title = Some(attr.value.to_string()),
                    _ => {}
                }
            }
            let content = element.content;
            let Some(raw_href) = href else {
                return Some(content.to_string());
            };
            let resolved = resolve_url(base_url_a.as_deref(), &raw_href);
            let escaped = resolved.replace('(', "\\(").replace(')', "\\)");
            let title_part = title.map_or(String::new(), |t| {
                format!(" \"{}\"", t.replace('"', "\\\""))
            });
            Some(format!("[{content}]({escaped}{title_part})"))
        })
        // Rewrite <img src> to absolute URLs when a base is available.
        .add_handler(vec!["img"], move |element: htmd::Element| {
            let mut src: Option<String> = None;
            let mut alt: Option<String> = None;
            let mut title: Option<String> = None;
            for attr in element.attrs {
                match attr.name.local.as_ref() {
                    "src" | "href" => src = Some(attr.value.to_string()),
                    "alt" => alt = Some(attr.value.to_string()),
                    "title" => title = Some(attr.value.to_string()),
                    _ => {}
                }
            }
            let src = src?;
            let resolved = resolve_url(base_url_img.as_deref(), &src);
            let escaped = resolved.replace('(', "\\(").replace(')', "\\)");
            let alt = alt.unwrap_or_default();
            let title_part = title.map_or(String::new(), |t| {
                format!(" \"{}\"", t.replace('"', "\\\""))
            });
            Some(format!("![{alt}]({escaped}{title_part})"))
        })
        .build();

    let md = converter
        .convert(html_str)
        .map_err(|e| anyhow!("HTML-to-Markdown conversion failed: {e}"))?;

    // Extract metadata from <head> and prepend as YAML frontmatter.
    let meta = extract_meta(html_str, base_url);
    let frontmatter = format_frontmatter(&meta);

    let body = format!("{frontmatter}{md}");

    Ok(Response::builder()
        .status(200)
        .header("content-type", "text/markdown; charset=utf-8")
        .body(body)
        .build())
}

/// Extract metadata from the HTML `<head>` for use as YAML frontmatter.
///
/// Pulls `<title>`, common `<meta>` tags (description, author, keywords,
/// Open Graph, Twitter Cards, article times), and `<link rel="canonical">`.
fn extract_meta(html: &str, base_url: Option<&str>) -> BTreeMap<String, String> {
    let doc = Html::parse_document(html);
    let mut meta = BTreeMap::new();

    // <title>
    if let Ok(sel) = Selector::parse("title") {
        if let Some(el) = doc.select(&sel).next() {
            let t = el.text().collect::<String>();
            let t = t.trim();
            if !t.is_empty() {
                meta.insert("title".into(), t.to_string());
            }
        }
    }

    // <link rel="canonical">
    if let Ok(sel) = Selector::parse(r#"link[rel="canonical"]"#) {
        if let Some(el) = doc.select(&sel).next() {
            if let Some(href) = el.value().attr("href") {
                let href = href.trim();
                if !href.is_empty() {
                    meta.insert("url".into(), resolve_url(base_url, href));
                }
            }
        }
    }

    // <meta name="..." content="...">  and  <meta property="..." content="...">
    if let Ok(sel) = Selector::parse("meta[content]") {
        for el in doc.select(&sel) {
            let content = el.value().attr("content").unwrap_or_default().trim();
            if content.is_empty() {
                continue;
            }
            let content = content.to_string();

            // key is either the `name` or `property` attribute
            let key = el
                .value()
                .attr("name")
                .or_else(|| el.value().attr("property"))
                .unwrap_or_default()
                .to_lowercase();

            match key.as_str() {
                // Basic meta
                "description" => {
                    meta.entry("description".into()).or_insert(content);
                }
                "author" => {
                    meta.entry("author".into()).or_insert(content);
                }
                "keywords" => {
                    meta.entry("keywords".into()).or_insert(content);
                }

                // Open Graph
                "og:title" => {
                    // OG title wins over <title> if present
                    meta.insert("title".into(), content);
                }
                "og:description" => {
                    meta.entry("description".into()).or_insert(content);
                }
                "og:image" => {
                    meta.insert(
                        "image".into(),
                        resolve_url(base_url, &content),
                    );
                }
                "og:url" => {
                    meta.entry("url".into()).or_insert(content);
                }
                "og:type" => {
                    meta.insert("type".into(), content);
                }
                "og:site_name" => {
                    meta.insert("site_name".into(), content);
                }
                "og:locale" => {
                    meta.insert("locale".into(), content);
                }

                // Twitter Cards
                "twitter:title" => {
                    meta.entry("title".into()).or_insert(content);
                }
                "twitter:description" => {
                    meta.entry("description".into()).or_insert(content);
                }
                "twitter:image" => {
                    meta.entry("image".into())
                        .or_insert_with(|| resolve_url(base_url, &content));
                }

                // Article timestamps
                "article:published_time" => {
                    meta.insert("published".into(), content);
                }
                "article:modified_time" => {
                    meta.insert("modified".into(), content);
                }

                _ => {}
            }
        }
    }

    meta
}

/// Format a metadata map as a YAML frontmatter block (`---\n...\n---\n`).
/// Returns an empty string when there is no metadata.
fn format_frontmatter(meta: &BTreeMap<String, String>) -> String {
    if meta.is_empty() {
        return String::new();
    }

    // Preferred key order — anything not listed here comes at the end
    // in BTreeMap's natural alphabetical order.
    const ORDER: &[&str] = &[
        "title",
        "description",
        "image",
        "url",
        "author",
        "published",
        "modified",
        "type",
        "site_name",
        "locale",
        "keywords",
    ];

    let mut lines = Vec::with_capacity(meta.len());

    // Emit keys in preferred order first.
    for &key in ORDER {
        if let Some(value) = meta.get(key) {
            lines.push(format!("{key}: {value}"));
        }
    }

    // Emit any remaining keys we didn't explicitly order.
    for (key, value) in meta {
        if !ORDER.contains(&key.as_str()) {
            lines.push(format!("{key}: {value}"));
        }
    }

    format!("---\n{}\n---\n\n", lines.join("\n"))
}

/// Resolve a potentially relative URL against an optional base URL.
/// If the href is already absolute, no base is given, or the base can't be
/// parsed, return the href as-is.
fn resolve_url(base: Option<&str>, href: &str) -> String {
    // Already absolute.
    if href.starts_with("http://") || href.starts_with("https://") || href.starts_with("//") {
        return href.to_string();
    }
    // Fragment-only or data/mailto/javascript — leave as-is.
    if href.starts_with('#')
        || href.starts_with("data:")
        || href.starts_with("mailto:")
        || href.starts_with("javascript:")
    {
        return href.to_string();
    }
    let Some(base) = base else {
        return href.to_string();
    };
    url::Url::parse(base)
        .and_then(|b| b.join(href))
        .map(|u| u.to_string())
        .unwrap_or_else(|_| href.to_string())
}
