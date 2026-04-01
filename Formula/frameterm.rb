class Frameterm < Formula
  desc "TUI automation for AI agents with video recording"
  homepage "https://github.com/stefanmatar/frameterm"
  version "1.2.1"
  license "MIT"

  depends_on "ffmpeg"

  on_macos do
    on_arm do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1.2.1/frameterm-macOS-arm64.tar.gz"
      sha256 "7e985690c957b9cab41b74c902db4c43ad19ef0d7e93008814abbfad9d7820f6"
    end
    on_intel do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1.2.1/frameterm-macOS-x86_64.tar.gz"
      sha256 "769bbf9f5253812a65c4f72bce956a829473de757aeda872c0934695148c1f5b"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1.2.1/frameterm-Linux-gnu-arm64.tar.gz"
      sha256 "c048869b1fb680283a4adf1e75fe750a8016054e1dfac7fb7b487e2b9d47fa0f"
    end
    on_intel do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1.2.1/frameterm-Linux-gnu-x86_64.tar.gz"
      sha256 "3f8b8975e6642ae295e119b14f9ff2cd266c3d7e2363792f43dc9fd061a8fc80"
    end
  end

  def install
    bin.install "frameterm"
  end

  test do
    assert_match "frameterm", shell_output("#{bin}/frameterm --help 2>&1", 2)
  end
end
