class Metsavaluaatio < Formula
  desc "Työkalu metsän tietojen analysointiin"
  homepage "https://github.com/jussinokelainen/metsavaluaatio"
  url "https://github.com/jussinokelainen/metsavaluaatio/archive/refs/tags/0.2.2.tar.gz"
  sha256 "e3d6490162658f8cd4f8903d282ef170717d5a5a4f4fb3b8bae88d79f81c1f22"
  license "AGPL-3.0-only"

  depends_on "rust" => :build
  depends_on "cmake" => :build
  depends_on "sqlite"

  def install
    system "cargo", "install", *std_cargo_args
  end
end
