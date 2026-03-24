package main

import (
	"fmt"
	"log"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	"github.com/getlantern/systray"
)

var scriptDir string

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

	rebuildMenu()
}

func onExit() {}

func rebuildMenu() {
	systray.ResetMenu()

	// Per-browser status
	detected := DetectedBrowsers()
	if len(detected) == 0 {
		item := systray.AddMenuItem("No supported browsers found", "")
		item.Disable()
	} else {
		for _, b := range detected {
			var label string
			if b.Patched() {
				label = fmt.Sprintf("\u2713 %s — Patched", b.DisplayName)
			} else {
				label = fmt.Sprintf("\u2717 %s — Not Patched", b.DisplayName)
			}
			item := systray.AddMenuItem(label, b.InstallPath)
			item.Disable()
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
		rebuildMenu()
	})
	if err != nil {
		log.Printf("Warning: could not start file watcher: %v", err)
	}

	go func() {
		for {
			select {
			case <-mPatch.ClickedCh:
				runPrivileged(filepath.Join(scriptDir, "fix-drm.sh"), "--force")
				rebuildMenu()

			case <-mUpdate.ClickedCh:
				_, err := shell("bash", filepath.Join(scriptDir, "download-widevine.sh"), "--force")
				if err == nil {
					runPrivileged(filepath.Join(scriptDir, "fix-drm.sh"), "--force")
				}
				rebuildMenu()

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

// --- Privilege escalation ---

func runPrivileged(script string, args ...string) {
	cmdArgs := []string{script}
	cmdArgs = append(cmdArgs, args...)
	fullCmd := "bash " + strings.Join(cmdArgs, " ")

	// Try pkexec first (graphical auth dialog), fall back to sudo
	if _, err := exec.LookPath("pkexec"); err == nil {
		_, _ = shell("pkexec", "bash", script)
	} else {
		_, _ = shell("sudo", "bash", script)
	}
	_ = fullCmd
}

func autoPatch() {
	runPrivileged(filepath.Join(scriptDir, "fix-drm.sh"))
}

// --- Shell execution ---

func shell(name string, args ...string) (string, error) {
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

// --- Embedded icon (16x16 minimal PNG) ---
// A small neon-tube-shaped icon. This is a placeholder; replace with a proper
// PNG asset for production.

var iconData = func() []byte {
	// Minimal 16x16 RGBA PNG: neon tube shape (white on transparent)
	// In production, embed a real icon via go:embed or load from file.
	// For now, use a 1x1 transparent pixel so the app doesn't crash.
	return []byte{
		0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // PNG signature
		0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
		0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1
		0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, // RGBA
		0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, // IDAT chunk
		0x54, 0x78, 0x9c, 0x62, 0x00, 0x00, 0x00, 0x02,
		0x00, 0x01, 0xe5, 0x27, 0xde, 0xfc, 0x00, 0x00, // compressed data
		0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, // IEND chunk
		0x60, 0x82,
	}
}()
