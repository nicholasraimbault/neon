import AppKit

// Browser configs: (display name, app name, framework name)
let browsers: [(String, String, String)] = [
    ("Helium",   "Helium",   "Helium Framework"),
    ("Thorium",  "Thorium",  "Thorium Framework"),
    ("Chromium", "Chromium", "Chromium Framework"),
]

class AppDelegate: NSObject, NSApplicationDelegate {
    var statusItem: NSStatusItem!
    var watchers: [FileWatcher] = []
    let bundlePath = Bundle.main.resourcePath ?? "."
    let agentPlist = NSHomeDirectory() + "/Library/LaunchAgents/com.neon.app.plist"

    func applicationDidFinishLaunching(_ notification: Notification) {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        if let button = statusItem.button {
            button.image = NSImage(systemSymbolName: "play.shield.fill", accessibilityDescription: "Neon")
        }
        rebuildMenu()
        startWatching()
    }

    // MARK: - Menu

    func rebuildMenu() {
        let menu = NSMenu()

        // Per-browser status lines
        var anyDetected = false
        for (display, app, fw) in browsers {
            let path = "/Applications/\(app).app"
            guard FileManager.default.fileExists(atPath: path) else { continue }
            anyDetected = true
            let patched = isPatched(app: app, framework: fw)
            let item = NSMenuItem(title: "\(display) — \(patched ? "Patched" : "Not Patched")", action: nil, keyEquivalent: "")
            item.isEnabled = false
            if patched {
                item.image = NSImage(systemSymbolName: "checkmark.circle.fill", accessibilityDescription: "Patched")
            } else {
                item.image = NSImage(systemSymbolName: "xmark.circle", accessibilityDescription: "Not Patched")
            }
            menu.addItem(item)
        }

        if !anyDetected {
            let item = NSMenuItem(title: "No supported browsers found", action: nil, keyEquivalent: "")
            item.isEnabled = false
            menu.addItem(item)
        }

        menu.addItem(NSMenuItem.separator())
        menu.addItem(NSMenuItem(title: "Patch Now", action: #selector(patchNow), keyEquivalent: "p"))
        menu.addItem(NSMenuItem(title: "Update Widevine", action: #selector(updateWidevine), keyEquivalent: "u"))
        menu.addItem(NSMenuItem.separator())

        let loginItem = NSMenuItem(title: "Launch at Login", action: #selector(toggleLaunchAtLogin), keyEquivalent: "")
        loginItem.state = FileManager.default.fileExists(atPath: agentPlist) ? .on : .off
        menu.addItem(loginItem)

        menu.addItem(NSMenuItem.separator())
        menu.addItem(NSMenuItem(title: "Quit Neon", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q"))

        statusItem.menu = menu
    }

    // MARK: - Browser status

    func isPatched(app: String, framework: String) -> Bool {
        let fwPath = "/Applications/\(app).app/Contents/Frameworks/\(framework).framework/Versions"
        guard let versions = try? FileManager.default.contentsOfDirectory(atPath: fwPath) else { return false }
        guard let ver = versions.first(where: { $0.first?.isNumber == true }) else { return false }
        let cdmPath = "\(fwPath)/\(ver)/Libraries/WidevineCdm"
        return FileManager.default.fileExists(atPath: cdmPath)
    }

    // MARK: - Actions

    @objc func patchNow() {
        runPrivileged("bash '\(bundlePath)/fix-drm.sh' --force")
        rebuildMenu()
    }

    @objc func updateWidevine() {
        let result = shell("bash '\(bundlePath)/download-widevine.sh' --force")
        if result.exitCode == 0 {
            runPrivileged("bash '\(bundlePath)/fix-drm.sh' --force")
        } else {
            showAlert(title: "Update Failed", message: result.output)
        }
        rebuildMenu()
    }

    @objc func toggleLaunchAtLogin() {
        if FileManager.default.fileExists(atPath: agentPlist) {
            shell("launchctl unload '\(agentPlist)' 2>/dev/null; rm -f '\(agentPlist)'")
        } else {
            let appPath = Bundle.main.bundlePath
            let plist = """
            <?xml version="1.0" encoding="UTF-8"?>
            <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
            <plist version="1.0">
            <dict>
                <key>Label</key>
                <string>com.neon.app</string>
                <key>ProgramArguments</key>
                <array>
                    <string>\(appPath)/Contents/MacOS/Neon</string>
                </array>
                <key>RunAtLoad</key>
                <true/>
            </dict>
            </plist>
            """
            let dir = NSHomeDirectory() + "/Library/LaunchAgents"
            try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
            try? plist.write(toFile: agentPlist, atomically: true, encoding: .utf8)
            shell("launchctl load '\(agentPlist)'")
        }
        rebuildMenu()
    }

    // MARK: - File watching

    func startWatching() {
        for (_, app, _) in browsers {
            let path = "/Applications/\(app).app"
            let watcher = FileWatcher(path: path) { [weak self] in
                // Debounce: wait for the update to finish
                DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
                    self?.autoPatch()
                }
            }
            watcher.start()
            watchers.append(watcher)
        }
    }

    func autoPatch() {
        runPrivileged("bash '\(bundlePath)/fix-drm.sh'")
        rebuildMenu()
    }

    // MARK: - Helpers

    func runPrivileged(_ command: String) {
        let escaped = command.replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
        let script = "do shell script \"\(escaped)\" with administrator privileges"
        let result = shell("osascript -e '\(script)'")
        if result.exitCode != 0 && !result.output.isEmpty {
            showAlert(title: "Error", message: result.output)
        }
    }

    @discardableResult
    func shell(_ command: String) -> (output: String, exitCode: Int32) {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/zsh")
        process.arguments = ["-c", command]
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = pipe
        try? process.run()
        process.waitUntilExit()
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        let output = String(data: data, encoding: .utf8) ?? ""
        return (output.trimmingCharacters(in: .whitespacesAndNewlines), process.terminationStatus)
    }

    func showAlert(title: String, message: String) {
        let alert = NSAlert()
        alert.messageText = title
        alert.informativeText = message
        alert.alertStyle = .warning
        alert.runModal()
    }
}

// MARK: - File Watcher

class FileWatcher {
    private var source: DispatchSourceFileSystemObject?
    private var fileDescriptor: Int32 = -1
    private let path: String
    private let onChange: () -> Void

    init(path: String, onChange: @escaping () -> Void) {
        self.path = path
        self.onChange = onChange
    }

    func start() {
        fileDescriptor = open(path, O_EVTONLY)
        guard fileDescriptor >= 0 else { return }
        source = DispatchSource.makeFileSystemObjectSource(
            fileDescriptor: fileDescriptor,
            eventMask: [.write, .delete, .rename, .attrib],
            queue: .main
        )
        source?.setEventHandler { [weak self] in
            self?.onChange()
        }
        source?.setCancelHandler { [weak self] in
            if let fd = self?.fileDescriptor, fd >= 0 { close(fd) }
        }
        source?.resume()
    }

    func stop() {
        source?.cancel()
        source = nil
    }

    deinit { stop() }
}

// MARK: - Entry point

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let delegate = AppDelegate()
app.delegate = delegate
app.run()
