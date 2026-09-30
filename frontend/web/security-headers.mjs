export function consoleHeaders(development = false, nonce) {
  return {
    "Content-Security-Policy": [
      "default-src 'none'",
      `script-src 'self'${development ? " 'unsafe-inline'" : ""}`,
      "script-src-attr 'none'",
      `style-src 'self'${development ? " 'unsafe-inline'" : ""}`,
      ...(nonce ? [`style-src-elem 'self' 'nonce-${nonce}'`] : []),
      "style-src-attr 'none'",
      "img-src 'self'",
      "font-src 'self'",
      `connect-src 'self'${development ? " ws: wss:" : ""}`,
      "base-uri 'none'",
      "form-action 'self'",
      "frame-ancestors 'none'",
    ].join("; "),
    "X-Frame-Options": "DENY",
    "X-Content-Type-Options": "nosniff",
    "Referrer-Policy": "no-referrer",
    "Permissions-Policy": "camera=(), microphone=(), geolocation=()",
    "Cache-Control": "no-store",
  };
}
