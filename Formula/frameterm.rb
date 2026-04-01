class Frameterm < Formula
  desc "TUI automation for AI agents with video recording"
  homepage "https://github.com/stefanmatar/frameterm"
  version "1"
  license "MIT"

  depends_on "ffmpeg"

  on_macos do
    on_arm do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1/frameterm-macOS-arm64.tar.gz"
      sha256 "f4c4f6de58e762bf1a757235cbf48c543761395d8a7a328ccfd50fbcfc51b87b"
    end
    on_intel do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1/frameterm-macOS-x86_64.tar.gz"
      sha256 "9e3554d3e2a5745f3596ce55eb8a0ccf431594c1dda4b8ccac8e30c8e81205bf"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1/frameterm-Linux-gnu-arm64.tar.gz"
      sha256 "81fbca83ff8c3f00a89a3b178a9b10fd6301bf894aabb5b3c6a6430a9e517164"
    end
    on_intel do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1/frameterm-Linux-gnu-x86_64.tar.gz"
      sha256 "ffe1a2c9af9063f8f32648ca4cdc8876d6e5861b550896dc9834a17e2d780db9"
    end
  end

  def install
    bin.install "frameterm"
  end

  test do
    assert_match "frameterm", shell_output("#{bin}/frameterm --help 2>&1", 2)
  end
end
