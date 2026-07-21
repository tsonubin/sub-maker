# frozen_string_literal: true

# Homebrew formula (generated). Install:
#   brew tap tsonubin/sub-maker https://github.com/tsonubin/sub-maker
#   brew install sub-maker
class SubMaker < Formula
  desc "TUI to set up a sing-box proxy server and Clash subscription"
  homepage "https://github.com/tsonubin/sub-maker"
  version "0.2.0"
  license :cannot_represent

  livecheck do
    url :stable
    regex(/^v?(\d+(?:\.\d+)+)$/i)
  end

  on_macos do
    on_arm do
      url "https://github.com/tsonubin/sub-maker/releases/download/v0.2.0/sub-maker-darwin-arm64"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
    on_intel do
      url "https://github.com/tsonubin/sub-maker/releases/download/v0.2.0/sub-maker-darwin-amd64"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/tsonubin/sub-maker/releases/download/v0.2.0/sub-maker-linux-arm64"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
    on_intel do
      url "https://github.com/tsonubin/sub-maker/releases/download/v0.2.0/sub-maker-linux-amd64"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
  end

  def install
    # GitHub asset names become the local filename when downloaded.
    binary = Dir["sub-maker-*"].first || "sub-maker"
    bin.install binary => "sub-maker"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/sub-maker --version")
  end

  def caveats
    <<~EOS
      sub-maker is primarily for Linux VPS setup (systemd, /etc paths).

      Production setup (Linux, root):
        sudo sub-maker --setup

      Demo / non-root:
        SUB_MAKER_DEMO=1 sub-maker --setup

      Full install docs: https://github.com/tsonubin/sub-maker/blob/main/INSTALL.md
    EOS
  end
end
