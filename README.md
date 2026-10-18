# NSWpedia Search

NSWpedia Search is a desktop app for Windows and macOS. It searches
NSWpedia for Nintendo Switch titles and shows the 1fichier download link for
each file.

![NSWpedia Search with results for "Hollow Knight" and three ready download links](docs/assets/readme/search.png)

## Download

Get the app from the
**[latest release](https://github.com/Skarian/nswpedia-search/releases/latest)**:

- Windows 10 or 11: `nswpedia-search-<version>-windows-x64-setup.exe`
- Mac with Apple silicon: `nswpedia-search-<version>-macos-arm64.dmg`
- Mac with an Intel processor: `nswpedia-search-<version>-macos-x64.dmg`

The app is not signed with a paid certificate. Windows and macOS show a
warning the first time you open it. The [installation guide](docs/install.md)
tells you what to do. It also explains upgrades, uninstalls and the app's
known limits.

## Find links

1. Type a title and press **Enter**.
2. The app opens the first result. Select a different result to see its files.
3. Each file gets a ✓ and a link when the link is ready.
4. Click **copy** for one link, or **copy all** for all ready links.

## Download files with Real-Debrid (optional)

With a [Real-Debrid](https://real-debrid.com) premium account, the app can
download the files for you.

1. Click **settings** (top right).
2. Paste your API token from
   [real-debrid.com/apitoken](https://real-debrid.com/apitoken).
3. Select a download folder. The default is your Downloads folder.
4. Click **save**.
5. Click **download** next to a file, or **download all**.

The app keeps the token in Windows Credential Manager or the macOS Keychain.
Click **↓** to see the download queue.

## More information

- [Install, upgrade and uninstall](docs/install.md)
- [Build, test and internals](docs/development.md)
- [CI and releases](docs/releases.md)

The license is MIT, as [Cargo.toml](Cargo.toml) declares.
