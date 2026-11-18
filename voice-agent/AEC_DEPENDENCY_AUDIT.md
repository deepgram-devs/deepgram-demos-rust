# AEC dependency audit

Workspace review date: 2026-11-17

The isolated AEC candidate graph audit below was last run on 2026-11-05. The workspace lockfile was refreshed and audited again on 2026-11-17.

## Candidate dependency

AEC candidate graph audit date: 2026-11-05

- Crate: `aec3 = "=0.4.0"`
- Cargo feature: optional `aec`, enabled by default
- Features: default features disabled; AEC3 linear pipeline only
- Registry metadata: `MIT OR BSD-3-Clause`; `aec3` 0.4.0
- Candidate graph audit: an isolated lockfile for this exact dependency and feature configuration passed with 13 packages and no findings against the 1,409-advisory RustSec database.

The candidate graph resolved `aec3 0.4.0`, `crossbeam-channel 0.5.17`, `crossbeam-utils 0.8.23`, `log 0.4.35`, `num-complex 0.4.6`, `num-traits 0.2.19`, `autocfg 1.5.1`, `rustfft 6.4.1`, `num-integer 0.1.47`, `primal-check 0.3.4`, `strength_reduce 0.2.4`, and `transpose 0.2.3`.

The workspace lockfile resolves the AEC path to `aec3 0.4.0`, `crossbeam-channel 0.5.17`, `crossbeam-utils 0.8.21`, `log 0.4.34`, `num-complex 0.4.6`, `num-traits 0.2.19`, `autocfg 1.5.0`, `rustfft 6.4.1`, `num-integer 0.1.46`, `primal-check 0.3.4`, `strength_reduce 0.2.4`, and `transpose 0.2.3`.

## Workspace audit

`cargo audit` scanned 1,054 locked packages against 1,527 advisories. It exits nonzero with 21 vulnerability advisories and 22 allowed warnings in other workspace dependency graphs. The current all-target Voice Agent graph was checked with `cargo tree -p voice-agent --all-features --target all` and does not contain the remaining vulnerable versions.

The Voice Agent release graph was updated to `openssl 0.10.82` and `openssl-sys 0.9.118` for the native-TLS WebSocket path, `rustix 1.1.6` for terminal/audio support, and `clipboard-win 5.4.3` for Windows clipboard support. These replace the previously audited OpenSSL vulnerabilities, Rustix unsoundness, and yanked clipboard package.

The remaining workspace advisory packages include `crossbeam-epoch 0.9.18`, `ctutils 0.4.2`, `h2 0.3.27` and `0.4.13`, `quinn-proto 0.11.15`, `quinn-udp 0.5.14`, `rkyv 0.7.46`, `rtrb 0.3.4`, `rustls 0.21.12`, `rustls-webpki 0.101.7`, and `webpki-root-certs 1.0.7`. Workspace warnings include unmaintained, unsound, and yanked packages outside the Voice Agent graph.
