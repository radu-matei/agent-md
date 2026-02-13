use anyhow::{anyhow, Context, Result};
use spin_sdk::http::{IntoResponse, Request, Response, Router};
use spin_sdk::http_component;

/// An Akamai Function that fetches a URL and converts the HTML content to Markdown.
///
/// This optimizes token transfer for AI agents by stripping away HTML markup
/// and returning clean Markdown content.
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

/// Handle POST requests — read the URL from the plain-text request body.
async fn handle_post(
    req: Request,
    _params: spin_sdk::http::Params,
) -> Result<impl IntoResponse> {
    let body = req.body();
    let url = std::str::from_utf8(body)
        .context("Request body is not valid UTF-8")?
        .trim()
        .to_string();

    if url.is_empty() {
        return Err(anyhow!(
            "POST body is empty. Send the target URL as the request body."
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

    // Convert the HTML body to Markdown.
    let html = resp.into_body();
    let html_str = String::from_utf8(html).context("Response body is not valid UTF-8")?;

    let converter = htmd::HtmlToMarkdown::builder()
        .skip_tags(vec!["script", "style", "noscript"])
        .build();

    let md = converter
        .convert(&html_str)
        .map_err(|e| anyhow!("HTML-to-Markdown conversion failed: {e}"))?;

    Ok(Response::builder()
        .status(200)
        .header("content-type", "text/markdown; charset=utf-8")
        .body(md)
        .build())
}
