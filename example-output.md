radu's blog

(function () { if (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches) { document.documentElement.classList.add('dark'); } })(); 

pre code.hljs { background: var(--code-bg) !important; padding: 0 !important; } pre { background: var(--code-bg) !important; padding: 1rem !important; margin: 1.5rem 0 !important; border-radius: var(--radius) !important; } 

document.addEventListener('DOMContentLoaded', function () { setTimeout(function () { document.querySelectorAll('pre code').forEach(function (block) { const content = block.innerHTML; const newContent = document.createTextNode(block.textContent); block.innerHTML = ''; block.appendChild(newContent); if (window.hljs) { window.hljs.highlightElement(block); } }); }, 100); }); 

function toggleSyntaxTheme(isDark) { let link = document.querySelector('link\[href\*="highlight.js"\]'); if (isDark) { link.href = 'https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/styles/github-dark.min.css'; } else { link.href = 'https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/styles/github.min.css'; } } if (window.matchMedia) { window.matchMedia('(prefers-color-scheme: dark)').addListener((e) => { toggleSyntaxTheme(e.matches); }); toggleSyntaxTheme(window.matchMedia('(prefers-color-scheme: dark)').matches); } 

[# Radu Matei](/)

  

I'm an engineer, founder, and CTO of [Fermyon](https://fermyon.com).

### Writing

*   [A practical guide to WebAssembly memory](/blog/practical-guide-to-wasm-memory/)
*   [From (C)Go to Rust: A practical guide to building shared and static libraries, linking, and FFI](/blog/from-go-to-rust-static-linking-ffi/)
*   [Introducing Fermyon Serverless AI](/blog/introducing-fermyon-serverless-ai/)
*   [Spin 1.0 — The Developer Tool for Serverless WebAssembly](/blog/introducing-spin-v1/)
*   [The Six Ways of Optimizing WebAssembly](/blog/six-ways-optimize-webassembly/)

[View all posts →](/blog)

### Projects

*   [@spinframework/spin](https://github.com/spinframework/spin)
    
    The CNCF framework for serverless functions with WebAssembly.
    
*   [fermyon.com](https://fermyon.com)
    
    Tools and platforms for running Wasm-powered functions at scale.
    
*   [@engineerd/setup-kind](https://github.com/engineerd/setup-kind)
    
    A GitHub Action to setup a Kubernetes cluster using Kind.
    

const isDarkMode = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches; if (isDarkMode) { document.documentElement.classList.add('dark'); } const nav = document.querySelector('nav'); const themeToggle = document.createElement('button'); themeToggle.setAttribute('aria-label', 'Toggle dark mode'); themeToggle.innerHTML = isDarkMode ? '☀️' : '🌙'; themeToggle.style.cssText = 'background: none; border: none; font-size: 1.2rem; cursor: pointer; margin-left: auto; padding: 0.5rem; color: var(--gray-600); position: absolute; top: 1.5rem; right: 1.5rem;'; themeToggle.addEventListener('click', () => { document.documentElement.classList.toggle('dark'); themeToggle.innerHTML = document.documentElement.classList.contains('dark') ? '☀️' : '🌙'; }); 

[About](/about/)•[Notes](/tags/notes/)