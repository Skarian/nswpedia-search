# Installing NSWpedia Search

Download from the
[latest release](https://github.com/Skarian/nswpedia-search/releases/latest).
You can also use the GitHub CLI:
`gh release download --repo Skarian/nswpedia-search`.

| Your computer | File |
| :--- | :--- |
| Windows 10 or 11 (64-bit) | `nswpedia-search-<version>-windows-x64-setup.exe` |
| Windows, no installation | `nswpedia-search-<version>-windows-x64-portable.zip` |
| Mac with Apple silicon (Apple menu → **About This Mac** shows **Chip: Apple M…**) | `nswpedia-search-<version>-macos-arm64.dmg` |
| Mac with an Intel processor (**Processor: Intel**) | `nswpedia-search-<version>-macos-x64.dmg` |

macOS 15 or newer is required.

## Why your system warns you

The builds are free and unsigned: no paid Windows code-signing certificate, no
Apple Developer ID and no notarization. Windows and macOS can't tell who made
the app, so they ask you to confirm the first time. Only approve files you
downloaded from this repository's releases. To check a download, compare it with
the release's `SHA256SUMS.txt`:

```powershell
Get-FileHash .\nswpedia-search-<version>-windows-x64-setup.exe   # Windows (PowerShell)
```

```sh
shasum -a 256 nswpedia-search-<version>-macos-arm64.dmg         # macOS (Terminal)
```

## Windows 10/11 (64-bit)

1. Download the setup file. Edge or Chrome may say it *isn't commonly
   downloaded*; choose **Keep** (in Edge: **…** → **Keep** → **Show more** →
   **Keep anyway**).
2. Run it. SmartScreen shows **Windows protected your PC** (*Unknown
   publisher*). Click **More info**, then **Run anyway**.
3. The installer doesn't need administrator rights. It installs for your user
   account into `%LOCALAPPDATA%\Programs\NSWpedia Search`, adds a Start menu
   shortcut and, if you tick the box, a desktop shortcut.

**Smart App Control** (Windows 11) blocks unsigned apps and doesn't offer
**Run anyway**. The only way to run the app then is to turn Smart App Control
off in **Windows Security → App & browser control → Smart App Control
settings**. That's a system-wide security setting, so decide for yourself
whether it's worth it.

**Portable version:** unzip the `-portable.zip` anywhere and run
`nswpedia-search.exe`. Keep `vcruntime140.dll` next to it. SmartScreen may show
the same warning. It shares settings and the saved token with an installed copy.

**Upgrade:** run the newer setup. It finds the existing installation, keeps the
same folder and shortcut choices, and offers to close the app if it's running.

**Uninstall:** **Settings → Apps → Installed apps → NSWpedia Search →
Uninstall** (or Control Panel).

## macOS 15 or newer

1. Open the DMG and drag **NSWpedia Search** onto **Applications**.
2. Open the app from Applications. macOS shows **"NSWpedia Search" Not
   Opened** (*Apple could not verify "NSWpedia Search" is free of malware…*).
   Click **Done**, not **Move to Trash**.
3. Open **System Settings → Privacy & Security**, scroll to **Security**, click
   **Open Anyway** next to *"NSWpedia Search" was blocked…*, enter your
   password, and click **Open Anyway** again. From then on it opens normally.

Since macOS 15, Control-clicking the app and choosing **Open** no longer skips
this check. If you prefer Terminal, this removes the download quarantine flag
instead:

```sh
xattr -dr com.apple.quarantine "/Applications/NSWpedia Search.app"
```

Files downloaded with `gh release download` normally aren't quarantined, so the
prompt usually doesn't appear for them.

**Upgrade:** drag the new version onto Applications and choose **Replace**. The
**Open Anyway** step is needed once for each downloaded version. Because each
build carries a new free (ad-hoc) signature, the first time a new version reads
your saved Real-Debrid token macOS may ask whether *NSWpedia Search* may use
confidential information stored in your keychain. Enter your login password and
click **Always Allow**. Your token is kept.

**Uninstall:** drag the app to the Trash.

## Your data

Installing, upgrading and uninstalling only touch the app itself. These stay
until you delete them:

| | Windows | macOS |
| :--- | :--- | :--- |
| Settings (`settings.json`) | `%APPDATA%\NSWpedia\NSWpedia Search\config` | `~/Library/Application Support/com.NSWpedia.NSWpedia-Search` |
| Search cache | `%LOCALAPPDATA%\NSWpedia\NSWpedia Search\cache` | `~/Library/Caches/com.NSWpedia.NSWpedia-Search` |
| Real-Debrid token | Credential Manager → Windows Credentials → `real-debrid-api-key.NSWpedia Search` | Keychain Access → login → `NSWpedia Search` |
| Downloads | your download folder | your download folder |

To remove the token from inside the app, open **settings** and press
**remove**.

There's no automatic updater. New versions appear on the
[releases page](https://github.com/Skarian/nswpedia-search/releases).
