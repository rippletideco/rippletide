/**
 * SSRF protection for user-controllable URLs.
 *
 * The CLI lets users override the backend API URL (`--backend-url`) and supply
 * the agent endpoint that gets fetched (`--agent`, templates, etc.). Without
 * validation, an attacker could point these at internal infrastructure or the
 * cloud metadata service (e.g. 169.254.169.254) to exfiltrate credentials or
 * pivot inside a network. `assertSafeBackendUrl` normalizes and validates a
 * user-provided URL, throwing a clear error when it looks unsafe.
 */

const LOOPBACK_HOSTNAMES = new Set(['localhost', '127.0.0.1', '::1', '[::1]']);

/**
 * Returns true when the hostname is an explicitly-allowed local loopback that
 * may be reached over plain http for local development.
 */
function isAllowedLoopback(hostname: string): boolean {
  return LOOPBACK_HOSTNAMES.has(hostname.toLowerCase());
}

/**
 * Detects private / internal / reserved IP literals that must be blocked to
 * prevent SSRF to internal infra or cloud metadata endpoints. Loopback is
 * handled separately by the caller so it can be allowed for local-http dev.
 */
function isBlockedAddressLiteral(hostname: string): boolean {
  const host = hostname.toLowerCase().replace(/^\[|\]$/g, '');

  // Unspecified address.
  if (host === '0.0.0.0' || host === '::' || host === '0:0:0:0:0:0:0:0') {
    return true;
  }

  // IPv4 literal: apply private/reserved range checks.
  const ipv4 = host.match(/^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/);
  if (ipv4) {
    const octets = ipv4.slice(1, 5).map((o) => Number(o));
    if (octets.some((o) => o > 255)) {
      // Not a valid IPv4 literal; let it fall through as a hostname.
      return false;
    }
    const [a, b] = octets;

    // 127.0.0.0/8 loopback (the allowed localhost case is filtered earlier).
    if (a === 127) return true;
    // 10.0.0.0/8 private.
    if (a === 10) return true;
    // 172.16.0.0/12 private.
    if (a === 172 && b >= 16 && b <= 31) return true;
    // 192.168.0.0/16 private.
    if (a === 192 && b === 168) return true;
    // 169.254.0.0/16 link-local (incl. 169.254.169.254 metadata).
    if (a === 169 && b === 254) return true;

    return false;
  }

  // Non-canonical numeric IPv4 encodings (decimal/octal/hex) are a classic SSRF
  // bypass for loopback/metadata (e.g. 2130706433, 0177.0.0.1, 0x7f.0.0.1).
  // Anything that is purely numeric/hex segments but did not match the clean
  // dotted-quad above is treated as an obfuscated address literal and blocked.
  if (/^0x[0-9a-f]+$/i.test(host) || /^\d+$/.test(host)) return true;
  if (/^(0x[0-9a-f]+|\d+)(\.(0x[0-9a-f]+|\d+)){1,3}$/i.test(host)) return true;

  // IPv6 literals (loopback ::1 is handled by the caller as allowed loopback).
  if (host.includes(':')) {
    // fc00::/7 unique local addresses (fc.. and fd..).
    if (/^f[cd][0-9a-f]*:/.test(host)) return true;
    // fe80::/10 link-local.
    if (/^fe[89ab][0-9a-f]*:/.test(host)) return true;
    // IPv4-mapped IPv6 in hex form, which is what new URL() normalizes
    // `::ffff:169.254.169.254` into (`::ffff:a9fe:a9fe`): decode the embedded
    // IPv4 from the two trailing hex groups and re-check its range.
    const mappedHex = host.match(/^::ffff:([0-9a-f]{1,4}):([0-9a-f]{1,4})$/i);
    if (mappedHex) {
      const hi = parseInt(mappedHex[1], 16);
      const lo = parseInt(mappedHex[2], 16);
      const v4 = `${(hi >> 8) & 0xff}.${hi & 0xff}.${(lo >> 8) & 0xff}.${lo & 0xff}`;
      if (isBlockedAddressLiteral(v4)) return true;
    }
    // IPv4-mapped IPv6 dotted-quad tail form (e.g. ::ffff:127.0.0.1), kept for
    // completeness in case a non-normalizing parser produces it.
    const mapped = host.match(/:((?:\d{1,3}\.){3}\d{1,3})$/);
    if (mapped && isBlockedAddressLiteral(mapped[1])) return true;
  }

  return false;
}

/**
 * Validates a user-provided backend / endpoint URL and returns it normalized.
 * Throws a descriptive Error when the URL is unsafe (SSRF protection).
 */
export function assertSafeBackendUrl(raw: string): string {
  let url: URL;
  try {
    url = new URL(raw);
  } catch {
    throw new Error(
      `Invalid URL "${raw}": could not be parsed. Provide a full URL such as https://example.com`
    );
  }

  const protocol = url.protocol.toLowerCase();
  if (protocol !== 'http:' && protocol !== 'https:') {
    throw new Error(
      `Unsupported URL protocol "${url.protocol}" in "${raw}". Only http and https are allowed (SSRF protection).`
    );
  }

  if (url.username || url.password) {
    throw new Error(
      `URL "${raw}" must not contain embedded credentials (SSRF protection).`
    );
  }

  const hostname = url.hostname;
  const loopback = isAllowedLoopback(hostname);

  // Require https everywhere except the explicitly-allowed local loopback,
  // where plain http is permitted for local development.
  if (protocol === 'http:' && !loopback) {
    throw new Error(
      `URL "${raw}" must use https. Plain http is only allowed for localhost (SSRF protection).`
    );
  }

  // Loopback over http is allowed; loopback over https is also fine.
  if (loopback) {
    return url.toString();
  }

  if (isBlockedAddressLiteral(hostname)) {
    throw new Error(
      `URL "${raw}" targets a private, internal, or reserved address (${hostname}) and is blocked (SSRF protection).`
    );
  }

  return url.toString();
}
