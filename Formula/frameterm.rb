class Frameterm < Formula
  desc "TUI automation for AI agents with video recording"
  homepage "https://github.com/stefanmatar/frameterm"
  version "1.3.0"
  license "MIT"

  depends_on "ffmpeg"

  on_macos do
    on_arm do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1.3.0/frameterm-macOS-arm64.tar.gz"
      sha256 "efb288650bb3b01af18b593a0fea856fcf13dec03972a6d26f1cb87ca6fbaaf7"
    end
    on_intel do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1.3.0/frameterm-macOS-x86_64.tar.gz"
      sha256 "429b9ad58585eabbc906ee479041a753ecf730e6d758219b9339f72f03f5a8ae"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1.3.0/frameterm-Linux-gnu-arm64.tar.gz"
      sha256 "5086482485e289660c25b81f03db0bd632aea0c04d02cbdfeb779cb59c1593a9"
    end
    on_intel do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1.3.0/frameterm-Linux-gnu-x86_64.tar.gz"
      sha256 "67b6b23ac315d98311ed8a733fc674096a95e4cd1c8c0eda1eeb1962b2e1a53f"
    end
  end

  def install
    bin.install "frameterm"
  end

  test do
    assert_match "frameterm", shell_output("#{bin}/frameterm --help 2>&1", 2)
  end
end
