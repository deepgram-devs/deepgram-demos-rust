# AEC dependency audit

Audit date: 2026-11-05

## Candidate dependency

- Crate: `aec3 = "=0.4.0"`
- Cargo feature: optional `aec`, enabled by default
- Features: default features disabled; AEC3 linear pipeline only
- Registry metadata: `MIT OR BSD-3-Clause`; `aec3` 0.4.0
- Candidate graph audit: an isolated lockfile for this exact dependency and feature configuration passed with 13 packages and no findings against the 1,409-advisory RustSec database.

The candidate graph resolved `aec3 0.4.0`, `crossbeam-channel 0.5.17`, `crossbeam-utils 0.8.23`, `log 0.4.35`, `num-complex 0.4.6`, `num-traits 0.2.19`, `autocfg 1.5.1`, `rustfft 6.4.1`, `num-integer 0.1.47`, `primal-check 0.3.4`, `strength_reduce 0.2.4`, and `transpose 0.2.3`.

The workspace lockfile resolves the AEC path to `aec3 0.4.0`, `crossbeam-channel 0.5.17`, `crossbeam-utils 0.8.21`, `log 0.4.34`, `num-complex 0.4.6`, `num-traits 0.2.19`, `autocfg 1.5.0`, `rustfft 6.4.1`, `num-integer 0.1.46`, `primal-check 0.3.4`, `strength_reduce 0.2.4`, and `transpose 0.2.3`.

## Workspace audit

`cargo audit` scanned 1,054 locked packages. It exits nonzero with 18 vulnerability advisories and 22 allowed warnings. The flagged packages are outside the voice-agent AEC-enabled dependency tree; `cargo tree --package voice-agent --features aec` shows the app using patched `tokio 1.54.1`, `rustls 0.23.46`, `rustls-webpki 0.103.16`, and `webpki-roots 1.0.11`.

Unresolved vulnerability packages: `crossbeam-epoch 0.9.18`, `ctutils 0.4.2`, `h2 0.3.27` and `0.4.13`, `quinn-proto 0.11.15` (five advisories), `rkyv 0.7.46`, `rtrb 0.3.4`, `rustls 0.21.12` (two advisories), `rustls-webpki 0.101.7` (four advisories), and `webpki-root-certs 1.0.7`.

Other audit warnings: unmaintained `async-std 1.13.2`, `dotenv 0.15.0`, `instant 0.1.13`, `paste 1.0.15`, `pin-utils 0.1.0`, `proc-macro-error2 2.0.1`, `rustybuzz 0.20.1`, `ttf-parser 0.25.1`, `unsafe-libyaml 0.2.11`, `zbus_names 4.3.1`, `zvariant 5.10.0`, and `zvariant_derive 5.10.0`; unsound `anyhow 1.0.102`, `event-listener 5.4.1`, `lru 0.12.5` (two advisories), and `memmap2 0.9.10`; yanked `async-executor 1.14.0`, `chacha20 0.10.0`, `clipboard-win 5.4.1`, and `spin 0.9.8` and `0.10.0`.
