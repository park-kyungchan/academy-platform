const allowed = new Map([['/', 'index.html'], ['/index.html', 'index.html'], ['/app.js', 'app.js'], ['/styles.css', 'styles.css']]);
const server = Bun.serve({ hostname: '127.0.0.1', port: 4173, fetch(request) {
  const path = allowed.get(new URL(request.url).pathname);
  if (request.method !== 'GET' || !path) return new Response('Not found', { status: 404 });
  return new Response(Bun.file(`dist/${path}`), { headers: { 'Cache-Control': 'no-store', 'Content-Security-Policy': "default-src 'self'; connect-src 'none'; img-src 'self' data:; style-src 'self'; script-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'" } });
} });
console.log(`Synthetic local preview: ${server.url}`);
