class Metsavaluaatio < Formula
  desc "Työkalu metsän tietojen analysointiin"
  homepage "https://github.com/jussinokelainen/metsavaluaatio"
  url "https://github.com/jussinokelainen/metsavaluaatio/archive/refs/tags/0.2.6.tar.gz"
  sha256 "f436e7291c7ac26d47aef6058648383da36003e0226feef4f4a005c5152279e4"
  license "AGPL-3.0-only"

  depends_on "rust" => :build
  depends_on "cmake" => :build
  depends_on "sqlite"

  def install
    system "cargo", "install", *std_cargo_args
  end
end
