# frozen_string_literal: true

# Homebrew formula (generated). Install:
#   brew tap tsonubin/sub-maker https://github.com/tsonubin/sub-maker
#   brew install sub-maker
class SubMaker < Formula
  desc "TUI to set up a sing-box proxy server and Clash subscription"
  homepage "https://github.com/tsonubin/sub-maker"
  version "0.2.1"
  license :cannot_represent

  livecheck do
    url :stable
    regex(/^v?(\d+(?:\.\d+)+)$/i)
  end

  on_macos do
    on_arm do
      url "https://github.com/tsonubin/sub-maker/releases/download/v0.2.1/sub-maker-darwin-arm64"
      sha256 "f2ef07ff37dae384dc44109b1963533e8d2d19a2fdab3ae29943b00cb55ba706"
    end
    on_intel do
      url "https://github.com/tsonubin/sub-maker/releases/download/v0.2.1/sub-maker-darwin-amd64"
      sha256 "978f9c117b24b4e5db2c8bbfcaaf3a885f91b2def6057cfd76b8755138a4f577"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/tsonubin/sub-maker/releases/download/v0.2.1/sub-maker-linux-arm64"
      sha256 "9edb738f6347405da452ee6e8760631b848fc2e908204efd6cc0c6f94e09320e"
    end
    on_intel do
      url "https://github.com/tsonubin/sub-maker/releases/download/v0.2.1/sub-maker-linux-amd64"
      sha256 "4cdb1a67faf430ac794fdfec69ab4398d839f96d292e232f79e8d2d713c028aa"
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
