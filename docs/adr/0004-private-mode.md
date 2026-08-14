# ADR 0004: Private Mode Cryptography

## Status

Accepted (MVP approach)

## Context

Private mode should hide content from the relay and originating IP from the gateway without inventing primitives. RFC 9458 Oblivious HTTP and RFC 9180 HPKE are the standards basis.

## Decision

1. Use the `hpke` crate (RFC 9180) with suite **X25519 / HKDF-SHA256 / AES-128-GCM**.
2. Apply application-level request/response framing inspired by OHTTP, with AAD binding protocol version, direction, key id, and gateway origin.
3. Defer full byte-compatible OHTTP encapsulation interoperability until vectors against an independent implementation pass; document status honestly.
4. Ship **buffered** private responses first. Protected streaming remains experimental and disabled.
5. Require external crypto review before labeling private mode production-stable in a 1.0 release.

## Consequences

- No custom KEMs/AEADs
- Clear review target
- Possible framing tweak later for OHTTP interoperability without changing HPKE suite
