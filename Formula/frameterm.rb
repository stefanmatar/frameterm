class Frameterm < Formula
  desc "TUI automation for AI agents with video recording"
  homepage "https://github.com/stefanmatar/frameterm"
  version "1.2.2"
  license "MIT"

  depends_on "ffmpeg"

  on_macos do
    on_arm do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1.2.2/frameterm-macOS-arm64.tar.gz"
      sha256 "cf66fd66dffedbfae2d5b1e012bf336d5d235a0f669ed5fca39aee9273c4e645"
    end
    on_intel do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1.2.2/frameterm-macOS-x86_64.tar.gz"
      sha256 "d5ff85223e2de8581764abcecfe1252b47db5d16704f5e3604a6128e1b01eca3"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1.2.2/frameterm-Linux-gnu-arm64.tar.gz"
      sha256 "004dffeed52c7617bb32d0fd9880b6a337f223ed75c48c5216eebb298a1c6a00"
    end
    on_intel do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1.2.2/frameterm-Linux-gnu-x86_64.tar.gz"
      sha256 "6276452bc77966bd54fb89b4a026bffca8cf20f02e055d5fd09eb9647f0ecef2"
    end
  end

  def install
    bin.install "frameterm"
  end

  test do
    assert_match "frameterm", shell_output("#{bin}/frameterm --help 2>&1", 2)
  end
end
