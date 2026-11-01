class Metsavaluaatio < Formula
  desc "Työkalu metsän tietojen analysointiin"
  homepage "https://github.com/jussinokelainen/metsavaluaatio"
  url "https://github.com/jussinokelainen/metsavaluaatio/archive/refs/tags/0.2.4.tar.gz"
  sha256 "a2e6e363454084958909325999bbd9c1077c199213fa7ee28ea1cb90a4318c9f"
  license "AGPL-3.0-only"

  depends_on "rust" => :build
  depends_on "cmake" => :build
  depends_on "sqlite"

  def install
    system "cargo", "install", *std_cargo_args
  end
end
