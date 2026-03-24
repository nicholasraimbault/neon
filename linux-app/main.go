package main

import (
	_ "embed"
	"fmt"
	"log"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	"github.com/getlantern/systray"
)

var scriptDir string

// Menu items stored at package level so updateStatus() can modify them.
var browserItems []*systray.MenuItem
var browserRefs []Browser

func main() {
	// Resolve script directory: look next to binary, then fall back to /usr/lib/neon
	exe, err := os.Executable()
	if err == nil {
		candidate := filepath.Dir(exe)
		if _, err := os.Stat(filepath.Join(candidate, "fix-drm.sh")); err == nil {
			scriptDir = candidate
		}
	}
	if scriptDir == "" {
		scriptDir = "/usr/lib/neon"
	}

	systray.Run(onReady, onExit)
}

func onReady() {
	systray.SetTitle("Neon")
	systray.SetTooltip("Neon — DRM Fixer")
	systray.SetIcon(iconData)

	// Per-browser status items (built once, titles updated in-place)
	detected := DetectedBrowsers()
	if len(detected) == 0 {
		item := systray.AddMenuItem("No supported browsers found", "")
		item.Disable()
	} else {
		for _, b := range detected {
			item := systray.AddMenuItem(statusLabel(b), b.InstallPath)
			item.Disable()
			browserItems = append(browserItems, item)
			browserRefs = append(browserRefs, b)
		}
	}

	systray.AddSeparator()

	mPatch := systray.AddMenuItem("Patch Now", "Patch all detected browsers")
	mUpdate := systray.AddMenuItem("Update Widevine", "Re-download WidevineCdm and patch")

	systray.AddSeparator()

	mLogin := systray.AddMenuItemCheckbox("Launch at Login", "Start Neon on boot", autostartEnabled())

	systray.AddSeparator()

	mQuit := systray.AddMenuItem("Quit Neon", "")

	// Start file watcher for auto-patching
	watcher, err := NewWatcher(func() {
		autoPatch()
		updateStatus()
	})
	if err != nil {
		log.Printf("Warning: could not start file watcher: %v", err)
	}

	go func() {
		for {
			select {
			case <-mPatch.ClickedCh:
				runPrivileged(filepath.Join(scriptDir, "fix-drm.sh"), "--force")
				updateStatus()

			case <-mUpdate.ClickedCh:
				_, err := runShell("bash", filepath.Join(scriptDir, "download-widevine.sh"), "--force")
				if err == nil {
					runPrivileged(filepath.Join(scriptDir, "fix-drm.sh"), "--force")
				}
				updateStatus()

			case <-mLogin.ClickedCh:
				toggleAutostart()
				if autostartEnabled() {
					mLogin.Check()
				} else {
					mLogin.Uncheck()
				}

			case <-mQuit.ClickedCh:
				if watcher != nil {
					watcher.Close()
				}
				systray.Quit()
				return
			}
		}
	}()
}

func onExit() {}

func statusLabel(b Browser) string {
	if b.Patched() {
		return fmt.Sprintf("\u2713 %s — Patched", b.DisplayName)
	}
	return fmt.Sprintf("\u2717 %s — Not Patched", b.DisplayName)
}

func updateStatus() {
	for i, b := range browserRefs {
		browserItems[i].SetTitle(statusLabel(b))
	}
}

// --- Privilege escalation ---

func runPrivileged(script string, args ...string) {
	allArgs := append([]string{script}, args...)

	// Try pkexec first (graphical auth dialog), fall back to sudo
	if _, err := exec.LookPath("pkexec"); err == nil {
		pkArgs := append([]string{"bash"}, allArgs...)
		runShell("pkexec", pkArgs...)
	} else {
		sudoArgs := append([]string{"bash"}, allArgs...)
		runShell("sudo", sudoArgs...)
	}
}

func autoPatch() {
	runPrivileged(filepath.Join(scriptDir, "fix-drm.sh"))
}

// --- Shell execution ---

func runShell(name string, args ...string) (string, error) {
	cmd := exec.Command(name, args...)
	out, err := cmd.CombinedOutput()
	return strings.TrimSpace(string(out)), err
}

// --- Launch at Login (XDG autostart) ---

const autostartDir = ".config/autostart"
const autostartFile = "neon.desktop"

func autostartPath() string {
	home, _ := os.UserHomeDir()
	return filepath.Join(home, autostartDir, autostartFile)
}

func autostartEnabled() bool {
	_, err := os.Stat(autostartPath())
	return err == nil
}

func toggleAutostart() {
	path := autostartPath()
	if autostartEnabled() {
		os.Remove(path)
		return
	}

	exe, err := os.Executable()
	if err != nil {
		log.Printf("Error getting executable path: %v", err)
		return
	}

	content := fmt.Sprintf(`[Desktop Entry]
Type=Application
Name=Neon
Comment=DRM Fixer for Chromium browsers
Exec=%s
Icon=neon
Terminal=false
StartupNotify=false
X-GNOME-Autostart-enabled=true
`, exe)

	os.MkdirAll(filepath.Dir(path), 0755)
	os.WriteFile(path, []byte(content), 0644)
}

// --- Embedded icon (neon tube 22x22 PNG) ---

//go:embed neon.png
var iconData []byte
