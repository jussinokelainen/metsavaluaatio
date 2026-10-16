class Metsavaluaatio < Formula
  desc "Työkalu metsän tietojen analysointiin"
  homepage "https://github.com/jussinokelainen/metsavaluaatio"
  url "https://github.com/jussinokelainen/metsavaluaatio/archive/refs/tags/0.1.3.tar.gz"
  sha256 "1e80bc240ffb803839ab02d9672580f0743540d0d261bb3f6a7f8afd9ae54aca"
  license "AGPL-3.0-only"

  depends_on "rust" => :build
  depends_on "cmake" => :build
  depends_on "sqlite"

  def install
    system "cargo", "install", *std_cargo_args
  end
end
