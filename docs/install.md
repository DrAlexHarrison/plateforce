# Installing plateforce

Every route below needs no compiler, no Python and no package manager, and every one of
them analyses your trace on your own machine.

Downloads are on the [releases page](https://github.com/DrAlexHarrison/plateforce/releases).
There is also a browser build at
[dralexharrison.github.io/plateforce](https://dralexharrison.github.io/plateforce), which
loads the file from your disk into the tab and sends nothing anywhere.

## Linux

**AppImage, and this is the one to take if you are not sure.** It needs no root and no
package manager, which matters on a machine somebody else administers.

```sh
chmod +x plateforce_<version>_amd64.AppImage
./plateforce_<version>_amd64.AppImage
```

**Debian and Ubuntu.** `sudo apt install ./plateforce_<version>_amd64.deb`

**Fedora and openSUSE.** `sudo dnf install ./plateforce-<version>-1.x86_64.rpm`

All three need glibc 2.35 or newer, which is Ubuntu 22.04, Debian 12 and anything later,
and they need the WebKitGTK runtime your desktop almost certainly already has. If the
application does not start, or you are on RHEL, Rocky or AlmaLinux, read the last section.

## macOS

Open `plateforce_<version>_universal.dmg` and drag plateforce to Applications. One file runs
on both Intel and Apple Silicon.

macOS 10.13 and newer. The application is signed with a Developer ID certificate and
notarised by Apple, and the notarisation ticket travels inside the file, so it opens with no
warning and with no network connection on first launch.

**Who signed it.** macOS reports the signer as Saturday Inc. plateforce is Apache-2.0,
authored by Alex Harrison, and is not a Saturday Inc product. It is signed under that
company's Apple Developer membership.

## Windows

Run `plateforce_<version>_x64-setup.exe`. It installs into your own user profile and does
not ask for an administrator.

Windows shows a blue dialog reading "Windows protected your PC" the first time, because the
installer carries no purchased certificate. Choose **More info**, then **Run anyway**.

On a Windows 11 machine that was clean-installed, Smart App Control may block the file
outright with no per-app override. Smart App Control cannot be enabled on an upgraded
machine, so this is rare; if you meet it, the last section is your route.

Windows 10 version 1809 and newer. It uses the WebView2 runtime, which ships with Windows 11
and arrived on Windows 10 through Windows Update.

## A machine that will not let you install anything

Enterprise Linux, an air-gapped analysis box, a locked-down laboratory workstation, a
computer where you are not an administrator and cannot become one.

Take one file, `plateforce-x86_64-linux-static` for a normal machine or
`plateforce-aarch64-linux-static` on arm64. It is statically linked, so it needs no glibc,
no runtime and no installation, and it runs on RHEL, Rocky and AlmaLinux, which never
shipped the WebKitGTK version the desktop applications link against.

```sh
chmod +x plateforce-x86_64-linux-static
./plateforce-x86_64-linux-static serve
```

It prints an address. Open it in the browser already on that machine, and the full interface
is there. The address is bound to the machine itself, so nothing on your network can reach
it, and no request leaves the computer.

`--port 8000` asks for a particular port instead of whichever one is free. `--open` asks
your browser to open the address for you.

The same binary is the command line program, so `./plateforce-x86_64-linux-static --help`
lists everything it does in a terminal.

## Python

```sh
pip install plateforce
```

The machine that installs it needs no compiler and no Rust toolchain: one wheel per platform
carries the engine already built, and the same engine the browser, the desktop application
and the command line run, so a number that differs between two of them is a build that
failed rather than a discrepancy to reconcile.

Python 3.11 and newer, on Linux, macOS and Windows. One abi3 wheel per platform covers 3.11
and every later version, and the registry travels inside the wheel, so the digest a result
reports names the same bytes on every machine that installed the same version.

`crates/plateforce-python/README.md` shows a worked analysis and what the result carries.

**`plateforce` is not `forceplate`.** The similarly named CRAN package analyses
posturography, centre-of-pressure measures from quiet standing. This one computes jump
kinetics from a vertical ground reaction force trace.
