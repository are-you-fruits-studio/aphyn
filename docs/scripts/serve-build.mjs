// Minimal static file server that faithfully emulates GitHub Pages project-site
// behavior for the Docusaurus `build/` output, including the embedded native
// rustdoc subtree under `api-reference/`.
//
// Why this exists instead of `docusaurus serve`:
//   `docusaurus serve` applies `trailingSlash: false` cleanup to *every* request,
//   which 301-redirects `*.html` URLs to extensionless paths AND strips the
//   `/aphyn/` baseUrl prefix. That breaks rustdoc's relative `.html`
//   navigation (All Items, module/struct links, source view) by bouncing it to
//   the Docusaurus home page. GitHub Pages does no such redirect: it serves
//   `*.html` files verbatim. This server matches GitHub Pages, so local preview
//   behaves exactly like production.
//
// Resolution order for a request path (after stripping the baseUrl prefix):
//   1. exact file               -> serve it            (rustdoc *.html, assets)
//   2. <path>.html              -> serve it            (Docusaurus extensionless pages)
//   3. <path>/index.html        -> serve it            (directory index)
//   4. otherwise                -> 404.html (status 404)

import http from 'node:http';
import { promises as fs } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const buildDir = path.resolve(scriptDir, '..', 'build');
const baseUrl = '/aphyn/'; // must match docusaurus.config.js `baseUrl`

const args = process.argv.slice(2);
const getArg = (name, fallback) => {
  const i = args.indexOf(name);
  return i !== -1 && args[i + 1] ? args[i + 1] : fallback;
};
const host = getArg('--host', '127.0.0.1');
const port = Number(getArg('--port', '3000'));

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.ico': 'image/x-icon',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.ttf': 'font/ttf',
  '.map': 'application/json; charset=utf-8',
  '.xml': 'application/xml; charset=utf-8',
  '.txt': 'text/plain; charset=utf-8',
};

const contentType = (filePath) => MIME[path.extname(filePath).toLowerCase()] || 'application/octet-stream';

async function tryFile(absPath) {
  try {
    const stat = await fs.stat(absPath);
    return stat.isFile() ? absPath : null;
  } catch {
    return null;
  }
}

async function isDirWithIndex(absPath) {
  try {
    const stat = await fs.stat(absPath);
    if (!stat.isDirectory()) return false;
    return Boolean(await tryFile(path.join(absPath, 'index.html')));
  } catch {
    return false;
  }
}

// Map a URL pathname to an action, mirroring GitHub Pages:
//   { kind: 'file', path }      -> serve the file
//   { kind: 'redirect' }        -> 302 to pathname + '/' (directory missing its slash)
//   null                        -> not found
async function resolve(pathname) {
  // Normalize the baseUrl prefix away; allow the bare prefix without slash too.
  let rel = pathname;
  if (rel.startsWith(baseUrl)) {
    rel = rel.slice(baseUrl.length);
  } else if (rel === baseUrl.slice(0, -1)) {
    rel = '';
  } else if (rel === '/') {
    rel = '';
  } else {
    // Outside the baseUrl: nothing to serve.
    return null;
  }

  rel = decodeURIComponent(rel).replace(/^\/+/, '');
  // Block path traversal.
  const candidate = path.normalize(path.join(buildDir, rel));
  if (!candidate.startsWith(buildDir)) return null;

  if (rel === '' || rel.endsWith('/')) {
    const index = await tryFile(path.join(candidate, 'index.html'));
    return index ? { kind: 'file', path: index } : null;
  }

  // Exact file, then the Docusaurus extensionless `<page>.html` form.
  const exact = (await tryFile(candidate)) || (await tryFile(`${candidate}.html`));
  if (exact) return { kind: 'file', path: exact };

  // A directory requested without a trailing slash: GitHub Pages 301-redirects to
  // add the slash so the page's relative links (../static.files/, sibling modules)
  // resolve correctly. We use 302 to avoid permanently caching the redirect.
  if (await isDirWithIndex(candidate)) return { kind: 'redirect' };

  return null;
}

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url, `http://${req.headers.host}`);
  const { pathname } = url;

  // A dev server must never be cached, otherwise stale pages/redirects linger.
  res.setHeader('Cache-Control', 'no-store');

  // Redirect root to the baseUrl, like a GitHub project site.
  if (pathname === '/') {
    res.statusCode = 302;
    res.setHeader('Location', baseUrl);
    res.end();
    return;
  }

  const result = await resolve(pathname);

  if (result && result.kind === 'redirect') {
    res.statusCode = 302;
    res.setHeader('Location', `${pathname}/${url.search}`);
    res.end();
    return;
  }

  if (result && result.kind === 'file') {
    const body = await fs.readFile(result.path);
    res.statusCode = 200;
    res.setHeader('Content-Type', contentType(result.path));
    res.end(body);
    return;
  }

  // Fall back to the Docusaurus 404 page.
  const notFound = await tryFile(path.join(buildDir, '404.html'));
  res.statusCode = 404;
  res.setHeader('Content-Type', 'text/html; charset=utf-8');
  res.end(notFound ? await fs.readFile(notFound) : 'Not Found');
});

try {
  await fs.access(buildDir);
} catch {
  console.error(`Build directory not found: ${buildDir}\nRun the Docusaurus build first.`);
  process.exit(1);
}

server.listen(port, host, () => {
  console.log(`Serving "${buildDir}" at: http://${host}:${port}${baseUrl}`);
  console.log('Press Ctrl+C to stop.');
});
