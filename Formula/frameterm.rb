class Frameterm < Formula
  desc "TUI automation for AI agents with video recording"
  homepage "https://github.com/stefanmatar/frameterm"
  version "1.2.3"
  license "MIT"

  depends_on "ffmpeg"

  on_macos do
    on_arm do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1.2.3/frameterm-macOS-arm64.tar.gz"
      sha256 "66fe48879adfc80ac480ab6312205af70d3ba172b34db81f998f1120fbcbd760"
    end
    on_intel do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1.2.3/frameterm-macOS-x86_64.tar.gz"
      sha256 "13f9ccd2c15b1e435f1d91b8b772a9b8aeb429a85082aebb72c54cb53f9949c6"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1.2.3/frameterm-Linux-gnu-arm64.tar.gz"
      sha256 "1dca00cfb539053ba0be37be0116b726a2c25277ef17eb50aa2ea469bfb6db99"
    end
    on_intel do
      url "https://github.com/stefanmatar/frameterm/releases/download/v1.2.3/frameterm-Linux-gnu-x86_64.tar.gz"
      sha256 "d3cf68e812c278fdf9e58a196e1b375a48512cded1906b29253f65e643a2b847"
    end
  end

  def install
    bin.install "frameterm"
  end

  test do
    assert_match "frameterm", shell_output("#{bin}/frameterm --help 2>&1", 2)
  end
end
