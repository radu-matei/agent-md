# agent-md

An Spin function that fetches a web page and returns its content as Markdown.

When AI agents browse the web they don't need HTML — they need the content.
agent-md sits between the agent and the origin, fetching the page, verifying it
is HTML, and converting it to clean Markdown. This dramatically reduces the
number of tokens transferred compared to raw HTML.

## Quick start

### Prerequisites

- [Spin](https://developer.fermyon.com/spin/v3/install) (>= 3.0)
- Rust with the `wasm32-wasip1` target (`rustup target add wasm32-wasip1`)

### Build

```bash
spin build
```

### Run locally

```bash
spin up
```

The function starts on `http://127.0.0.1:3000` by default.

## Usage

### GET — pass the URL as a query parameter

```bash
curl "http://127.0.0.1:3000/?url=https://example.com"
```

### POST — pass the URL in the request body

```bash
curl -X POST -d "https://example.com" http://127.0.0.1:3000/
```

### POST — pass HTML directly in the request body

Send the HTML you already have and skip the fetch. Set `Content-Type: text/html`:

```bash
curl -X POST \
  -H "Content-Type: text/html" \
  --data-binary @page.html \
  http://127.0.0.1:3000/
```

Because no origin URL is known in this mode, relative `<a href>` and `<img src>`
values (and metadata URLs like `og:image` / canonical) are emitted unchanged.
Absolute URLs are preserved as-is.

All three forms return the page content as Markdown with
`Content-Type: text/markdown; charset=utf-8`.

### Example output

```bash
$ curl -s "http://127.0.0.1:3000/?url=https://example.com"

# Example Domain

This domain is for use in illustrative examples in documents. You may use this
domain in literature without prior coordination or asking for permission.

[More information...](https://www.iana.org/domains/example)
```

### Use from Python (e.g. inside an AI agent)

```python
import requests

md = requests.get(
    "http://127.0.0.1:3000/",
    params={"url": "https://example.com"},
).text

print(md)
```

### Use from JavaScript / TypeScript

```typescript
const url = encodeURIComponent("https://example.com");
const res = await fetch(`http://127.0.0.1:3000/?url=${url}`);
const markdown = await res.text();
console.log(markdown);
```

## Error handling

| Scenario | Status | Body |
| --- | --- | --- |
| Missing `url` query parameter on GET | 500 | Error message with usage hint |
| Empty POST body (URL mode) | 500 | Error message with usage hint |
| Empty POST body (`Content-Type: text/html`) | 500 | Error message with usage hint |
| Upstream returns non-2xx (URL mode) | 502 | `Upstream returned HTTP <status> when fetching <url>` |
| Upstream response is not HTML (URL mode) | 400 | `The URL did not return HTML (Content-Type: ...)` |
| Unsupported HTTP method | 405 | Method Not Allowed |


## How it works

1. The function receives an HTTP request (GET or POST).
2. **URL mode** (GET, or POST without `Content-Type: text/html`): it fetches
   the page from the origin with `Accept: text/html`, following redirects, and
   verifies the response is a 2xx `text/html` body.
3. **HTML mode** (POST with `Content-Type: text/html`): the request body is
   used as the HTML directly — no fetch is performed.
4. It converts the HTML to Markdown using the
   [`htmd`](https://crates.io/crates/htmd) crate (built on
   [`html5ever`](https://crates.io/crates/html5ever)).
5. It returns the Markdown body to the caller.
