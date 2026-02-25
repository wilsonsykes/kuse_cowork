# Windows Deployment Guide

This guide covers how to distribute and run **Kuse Cowork by Wilson** on other Windows machines.

## 1. Build Artifacts to Distribute

Use installer outputs from:

- `src-tauri/target/release/bundle/nsis/Kuse Cowork_<version>_x64-setup.exe`
- `src-tauri/target/release/bundle/msi/Kuse Cowork_<version>_x64_en-US.msi`

Prefer the NSIS `setup.exe` for general users.  
Do **not** distribute raw `target/release/kuse-cowork.exe` for normal installation.

## 2. Target Machine Requirements

1. Windows 10/11 x64.
2. WebView2 runtime available (common on modern Windows).
3. If using Ollama mode:
   - Ollama installed and running.
   - Required model pulled locally (example: `ollama pull qwen2.5:7b-instruct`).
4. Network/firewall allows local endpoint access when needed (`http://localhost:11434`).

## 3. Install Steps (End User)

1. Copy installer to target machine.
2. Run installer:
   - NSIS: `Kuse Cowork_<version>_x64-setup.exe`
   - MSI: `Kuse Cowork_<version>_x64_en-US.msi`
3. If SmartScreen appears:
   - Click `More info` -> `Run anyway` (expected for unsigned builds).
4. Launch **Kuse Cowork** from Start menu or desktop shortcut.

## 4. First-Run Verification

1. Open app `Settings`.
2. Confirm model/provider configuration.
3. For Ollama:
   - Verify base URL is `http://localhost:11434/v1`.
   - Run `Test Connection`.
4. Run a simple task (example: list files in mounted folder) to verify tool execution.

## 5. Common Issues

## App does not start
- Ensure correct architecture (x64 build on x64 machine).
- Reinstall using the installer package (not raw exe).

## Ollama status issues
- Confirm `ollama serve` is running.
- Confirm model exists: `ollama list`.
- Retry connection test in app settings.

## SmartScreen warning
- Normal for unsigned binaries.
- To remove warnings for users, sign binaries with a trusted code-signing certificate.

## 6. Release Checklist

1. Update versions consistently:
   - `package.json`
   - `src-tauri/Cargo.toml`
   - `src-tauri/tauri.conf.json`
2. Build:
   - `npm run tauri:build`
3. Smoke test installer on a clean Windows VM.
4. Publish installer artifact(s) and release notes.

