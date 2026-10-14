class Metsavaluaatio < Formula
  desc "Työkalu metsän tietojen analysointiin"
  homepage "https://github.com/jussinokelainen/metsavaluaatio"
  url "https://github.com/jussinokelainen/metsavaluaatio/archive/refs/tags/0.1.1.tar.gz"
  sha256 "8930d6f372d42599d95d0876d1e4d5a52c08afe44e185da0a2694bac29464c7b"
  license "AGPL-3.0-only"

  depends_on "rust" => :build
  depends_on "cmake" => :build
  depends_on "sqlite"

  def install
    system "cargo", "install", *std_cargo_args
  end
end
