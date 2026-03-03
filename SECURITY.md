# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in MCE, please report it responsibly:

1. **Do NOT open a public issue**
2. Use [GitHub Security Advisories](https://github.com/yongsk0066/mce/security/advisories/new) to report privately
3. Or email **yongsk0066@gmail.com** with details

You should receive an acknowledgment within 48 hours.

## Scope

MCE runs entirely client-side (WASM in the browser or native CLI). Security concerns include:

- Malformed VFST dictionary files causing crashes or memory issues
- Input strings causing excessive CPU/memory usage (DoS)
- WASM sandbox escapes (unlikely but reportable)

## Dependency Auditing

MCE uses `cargo audit` in CI to check for known vulnerabilities in dependencies. Dependabot is enabled for automated dependency updates.
