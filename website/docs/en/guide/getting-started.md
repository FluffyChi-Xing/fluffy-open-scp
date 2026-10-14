---
title: Getting started
description: Install OpenSCP, configure the game directory and open your first package.
---
# Start with your first .package

This guide is based on the repository README. You need local SimCity (2013) files; OpenSCP does not include game assets.

## Install and launch

1. Visit [GitHub Releases](https://github.com/FluffyChi-Xing/fluffy-open-scp/releases) for available versions and installation notes.
2. On Windows, choose the MSI or NSIS installer offered by that release. Check the release notes for other operating systems.
3. Launch OpenSCP. Under **Settings → Game directory**, select the installation directory containing `SimCityData`, such as `D:\ea-games\SimCity`.
4. Wait for the package scan, then select a `.package` from the resource tree.

You can also double-click a `.package` in the package list without configuring a game directory first.

## Find an asset

Filter, search and paginate resources by **TGI (Type / Group / Instance)**. Selecting an asset opens the appropriate model, texture, Property, text, media or hex preview.

Start with textures and property lists, then open an RW4 model. Materials or decals with cross-package references may need the full game directory to resolve.

## Export and edit

- Export models as glTF 2.0 (`.glb`) or Wavefront `.obj`.
- Export textures as PNG / JPG / TGA / DDS and properties as JSON / TXT.
- The Locale editor exports overlay packages that replace original text resources using the same TGI.

Inspect the export before testing in the game. Keep original packages and a working copy so you can compare the results.

## Tools for audio and video

Media transcoding requires external tools that are not bundled with the installer. Other asset browsing features do not require them.

| Tool | Purpose | Download |
| --- | --- | --- |
| vgmstream | Decode Wwise audio to WAV | [Project website](https://github.com/vgmstream/vgmstream) |
| FFmpeg | Convert VP6 video to MP4 | [Official website](https://ffmpeg.org) |

Add them to your system `PATH`, or use the application's `Tools/vgmstream/` and `Tools/ffmpeg/` directories. The application identifies missing tools when needed.

## Next steps

Read the [workbench guide](./workbench) for asset browsing, map previews, Raster painting and Mod Studio. Check [development progress](../development) for current limitations.
