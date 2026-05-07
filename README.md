# Link Interceptor

A Windows desktop URL interceptor written in Rust with `eframe/egui`.

The app can be registered as a default browser candidate for `http` and `https`.
When another program opens a URL through the system browser, Windows can launch
this app with the URL as the first command-line argument. The app then lets you
edit, copy, favorite, save, and forward the URL to an installed browser or
another registered/custom handler.

## Features

- Intercept URL/deeplink passed as the first CLI argument.
- Edit and copy the intercepted URL.
- Automatically save intercepted URLs to YAML history.
- Add/remove favorites.
- Discover installed browsers from Windows registry.
- Discover URL protocol handlers such as `mailto:` or custom schemes.
- Configure custom applications and domain rules.
- Register/unregister the current executable under HKCU without admin rights.
- Open Windows Default Apps settings so the user can select this app.

## Build

```powershell
cargo build --release
```

Run without arguments to open the main UI:

```powershell
cargo run
```

Run with a URL to simulate interception:

```powershell
cargo run -- "https://example.com"
cargo run -- "mailto:test@example.com"
```

## Data Files

The app stores YAML files under:

```text
%APPDATA%\LinkInterceptor\config.yaml
%APPDATA%\LinkInterceptor\history.yaml
%APPDATA%\LinkInterceptor\favorites.yaml
```

## Windows Registration

Open the `Registration` tab and click `Register current exe`. This writes only
HKCU keys for the current executable path and registers the app as a browser
candidate. Windows 10/11 protects the actual default app choice, so the app does
not write `UserChoice`. Click `Open default apps settings` and choose Link
Interceptor in Windows settings.

If you move the portable executable, register it again from the new path.

## Current Scope

This is a v1 portable executable implementation. It intentionally does not yet
include an installer, auto-update, or single-instance IPC. Multiple launches can
open multiple windows.
