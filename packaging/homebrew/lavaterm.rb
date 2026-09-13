class Lavaterm < Formula
  desc "Terminal-native ambient lava lamp and metaball visualizer"
  homepage "https://github.com/ViolaPeracia/ZenLavaTerm"
  url "https://github.com/ViolaPeracia/ZenLavaTerm/archive/refs/tags/v1.0.1.tar.gz"
  sha256 "__SOURCE_SHA__"
  license "Apache-2.0"
  head "https://github.com/ViolaPeracia/ZenLavaTerm.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    output = shell_output("#{bin}/lavaterm --snapshot")
    assert_predicate output, :present?
  end
end
