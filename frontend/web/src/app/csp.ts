const value = document.querySelector<HTMLMetaElement>(
  'meta[name="csp-nonce"]',
)?.content;

export const cspNonce = value === "__GROVE_CSP_NONCE__" ? undefined : value;
