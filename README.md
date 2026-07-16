# Silvervine Experimental Bridge

> [!WARNING]
> This branch contains unsupported VM/GPU/TPM/Looking Glass research. It is excluded from Silvervine releases, installers, packages, and normal support. Do not use it as an installation source.

The bridge explores whether a Windows guest with GPU and TPM passthrough can provide playback modes unavailable to software-only Widevine L3. It is a research prototype, not a supported Silvervine feature.

## Status

- Requires advanced virtualization configuration and suitable multi-GPU hardware.
- May require proprietary guest software and separate licenses.
- Does not guarantee 4K, HDR, hardware-backed DRM, or compatibility with any streaming service.
- Has no stability, migration, security-update, or support commitment.
- Is maintained only when contributors choose to work on it.

The prototype source on this branch predates the current supported release and may contain historical names, paths, and interfaces. Those are implementation artifacts, not current product branding or compatibility promises.

## Supported Silvervine

For installation, usage, migration, security reporting, and supported-platform information, use the [`master`](https://github.com/nicholasraimbault/silvervine/tree/master) branch and [GitHub Releases](https://github.com/nicholasraimbault/silvervine/releases).

## Contributing

Contributor pull requests targeting `experimental-bridge` are welcome. Keep bridge-specific changes on this branch; do not add the bridge to release workflows or supported builds. Document hardware assumptions, failure modes, and licensing requirements without claiming successful DRM tiers or playback quality unless they have been reproduced.

## License

Silvervine is distributed under the [MIT License](LICENSE). Third-party hypervisors, guest operating systems, GPU drivers, Looking Glass, Widevine, and streaming services have their own terms and licenses.
