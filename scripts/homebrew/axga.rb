class Axga < Formula
  desc "4.7MB AI coding agent — runs on 1GB VPS"
  homepage "https://github.com/KJ-AIML/axga-harness-agent-rs"
  version "0.1.0"
  license "MIT"

  on_linux do
    url "https://github.com/KJ-AIML/axga-harness-agent-rs/releases/download/v#{version}/axga-v#{version}-x86_64-linux-musl"
    sha256 "REPLACE_WITH_ACTUAL_SHA256"
  end

  def install
    bin.install "axga-v#{version}-x86_64-linux-musl" => "axga"
  end

  test do
    system "#{bin}/axga", "--version"
  end
end
