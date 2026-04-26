# Velocity Scoop Deployment

This folder contains the Scoop manifest for publishing Velocity through a Scoop bucket.

## Local Test

From the repository root:

```powershell
scoop install .\velocity\deploy\scoop\velocity.json
velocity
scoop uninstall velocity
```

## Community Bucket Submission

Velocity is a GUI/tray utility, so the likely community bucket target is `ScoopInstaller/Extras`.

1. Fork `https://github.com/ScoopInstaller/Extras`.
2. Copy `velocity.json` into the fork's `bucket` directory.
3. Run the bucket checks from the fork.
4. Open a pull request with the manifest.

## Updating

For each Velocity release:

1. Build and publish `velocity-$version-windows-x64.zip` to the matching `velocity-v$version` GitHub release. The ZIP must include `deepgram-icon.ico` next to `velocity.exe` so the Scoop-created Start Menu shortcut uses the Deepgram icon.
2. Compute the SHA256 hash:

   ```powershell
   Get-FileHash .\target\release\velocity-$version-windows-x64.zip -Algorithm SHA256
   ```

3. Update `version`, `architecture.64bit.url`, and `architecture.64bit.hash` in `velocity.json`.
4. Verify with `scoop install .\velocity\deploy\scoop\velocity.json`.
