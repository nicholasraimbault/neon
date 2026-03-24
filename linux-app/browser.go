package main

import "os"

// Browser represents a Linux browser that may need WidevineCdm patching.
type Browser struct {
	DisplayName string
	InstallPath string
}

// These match the LINUX_BROWSERS array in fix-drm.sh.
var browsers = []Browser{
	{"Helium", "/opt/helium-browser-bin"},
	{"Thorium", "/opt/chromium.org/thorium"},
	{"Thorium", "/opt/thorium-browser"},
	{"ungoogled-chromium", "/usr/lib/chromium"},
	{"ungoogled-chromium", "/usr/lib64/chromium"},
	{"Chromium", "/usr/lib/chromium-browser"},
}

// Installed reports whether the browser directory exists.
func (b Browser) Installed() bool {
	info, err := os.Stat(b.InstallPath)
	return err == nil && info.IsDir()
}

// Patched reports whether WidevineCdm is installed in this browser.
func (b Browser) Patched() bool {
	_, err := os.Stat(b.InstallPath + "/WidevineCdm/manifest.json")
	return err == nil
}

// DetectedBrowsers returns only the browsers that are installed on this system.
func DetectedBrowsers() []Browser {
	var detected []Browser
	for _, b := range browsers {
		if b.Installed() {
			detected = append(detected, b)
		}
	}
	return detected
}
