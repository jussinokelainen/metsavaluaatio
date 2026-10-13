class Metsavaluaatio < Formula
  desc "Työkalu metsän tietojen analysointiin"
  homepage "https://github.com/jussinokelainen/metsavaluaatio"
  url "https://github.com/jussinokelainen/metsavaluaatio/archive/refs/tags/0.1.0.tar.gz"
  sha256 "2a29ce92d12ef0af9ecf32d24f381af18d5a4b34096b38913a364ef67a51919e"
  license "AGPL-3.0-only"

  depends_on "rust" => :build
  depends_on "cmake" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end
end
