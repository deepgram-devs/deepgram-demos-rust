# Velocity Scoop Deployment

Velocity is published through the repository-wide `dg` Scoop bucket at `scoop/bucket/velocity.json`.

The reusable release workflow builds both Windows x64 and ARM64 ZIPs with versioned names such as `velocity-0.5.1-x86_64-pc-windows-msvc.zip`, publishes them to the `velocity-v0.5.1` GitHub release, and commits the matching Scoop manifest hashes automatically. There is no separate manually maintained Velocity manifest.

After the manifest has been published, install it with:

```powershell
scoop bucket add dg https://github.com/deepgram-devs/deepgram-demos-rust
scoop install dg/velocity
```
