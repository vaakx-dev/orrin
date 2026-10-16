# Orrin

One Rust program that owns the user's projects: files, views, landing, builds and sync. Cargo workspace in `crates/`.

## Code rules
- Clean, small, readable: small files, clear names, organise by feature, split into modules as a feature grows.
- No code comments, no doc files besides this one. No backwards compatibility or migration code.
- Prefer well-known crates; add them to `[workspace.dependencies]` in the root `Cargo.toml` and use `x.workspace = true`.
- Small tests only: a few `#[test]` / `#[tokio::test]` per crate on the main path.
- `cargo build`, `cargo test` and `cargo clippy --workspace --all-targets -- -D warnings` must pass. Run `cargo fmt` on your crate.
- Commit only your own paths (`git add <paths>`, never `-A`); retry if `index.lock` is busy.

## Architecture
- `orrin-core` holds all shared types: ids, clock, errors, config, home, model, the API (`orrin_core::api`) and the service traits (`orrin_core::service`).
- Service crates depend only on `orrin-core`, never on each other. Each exposes `pub fn open(setup: &Setup) -> Result<Arc<dyn Trait>>` (`orrin-merge` exposes `pub fn all() -> Vec<Arc<dyn Merger>>`).
- Reach other services at request time through `setup.hub` (`hub.store`, `hub.projects`, ...). Never touch the hub inside `open`; it is set after every service is built. Background tasks start in `Service::start`.
- `orrin-server` wires services in `wiring.rs` and maps every request to one trait method in `routes.rs`.
- Errors use `orrin_core::Error` codes: `not_found`, `conflict`, `invalid`, `unavailable`, `unauthorized`, `internal`.
- Emit events through `hub.events.emit(Event::...)`.
- Never use port 4520 or `~/.orrin` in tests; use `--home <temp dir>` and port 0, and bind `127.0.0.1` there.
