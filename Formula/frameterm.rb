class Frameterm < Formula
  desc "TUI automation for AI agents with video recording"
  homepage "https://github.com/stefanmatar/frameterm"
  version "1.3.1"
  license "MIT"

  depends_on "ffmpeg"

  on_macos do
    on_arm do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1.3.1/frameterm-macOS-arm64.tar.gz"
      sha256 "378f57b6c90cea969186ac4622972cce9c65871906249d4d8ef6c4a945c1282d"
    end
    on_intel do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1.3.1/frameterm-macOS-x86_64.tar.gz"
      sha256 "6a6a5b15cf7d7f3dfd13fc79cb72f84a5526fd7161077ec0daa55d2187cf7319"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1.3.1/frameterm-Linux-gnu-arm64.tar.gz"
      sha256 "79ae36d6c541e0a6a25a36cf5f26576d95c5d65e2f0087a9aca764c5cd11e35d"
    end
    on_intel do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1.3.1/frameterm-Linux-gnu-x86_64.tar.gz"
      sha256 "0a64e47296628d2ca9855d1697d3f3862db047d08f0ff41f694bafea7a851888"
    end
  end

  def install
    bin.install "frameterm"
  end

  test do
    assert_match "frameterm", shell_output("#{bin}/frameterm --help 2>&1", 2)
  end
end
