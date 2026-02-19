class Neon < Formula
  desc "Fix DRM playback in Chromium-based browsers on macOS"
  homepage "https://github.com/nicholasraimbault/neon"
  url "https://github.com/nicholasraimbault/neon/archive/refs/tags/v1.0.0.tar.gz"
  sha256 "REPLACE_WITH_ACTUAL_SHA256_AFTER_TAGGING"
  license "MIT"

  depends_on :macos

  def install
    # Keep all scripts together in libexec so $SCRIPT_DIR resolution works
    libexec.install "fix-drm.sh"
    libexec.install "download-widevine.sh"
    libexec.install "install.sh"
    libexec.install "uninstall.sh"

    # Wrapper scripts in bin (added to PATH)
    (bin/"neon-install").write <<~SH
      #!/bin/bash
      exec "#{libexec}/install.sh" "$@"
    SH

    (bin/"neon-uninstall").write <<~SH
      #!/bin/bash
      exec "#{libexec}/uninstall.sh" "$@"
    SH

    (bin/"neon-patch").write <<~SH
      #!/bin/bash
      exec "#{libexec}/fix-drm.sh" "$@"
    SH

    (bin/"neon-update-widevine").write <<~SH
      #!/bin/bash
      exec "#{libexec}/download-widevine.sh" "$@"
    SH
  end

  def caveats
    <<~EOS
      Neon patches DRM (WidevineCdm) into Chromium-based browsers:
        Helium, Thorium, ungoogled-chromium

      After installing, run:
        neon-install

      This will download WidevineCdm, patch your browsers, and set up
      an auto-patch daemon. macOS will prompt for your password.

      Other commands:
        neon-patch              Patch browsers now
        neon-patch --force      Re-patch even if already patched
        neon-update-widevine    Download latest WidevineCdm
        neon-uninstall          Remove daemon and cached files

      For the menu bar app (Neon.app), download the DMG from:
        https://github.com/nicholasraimbault/neon/releases
    EOS
  end

  test do
    assert_predicate bin/"neon-install", :executable?
    assert_predicate bin/"neon-uninstall", :executable?
    assert_predicate bin/"neon-patch", :executable?
    assert_predicate bin/"neon-update-widevine", :executable?
    assert_predicate libexec/"fix-drm.sh", :executable?
    assert_predicate libexec/"download-widevine.sh", :executable?
  end
end
