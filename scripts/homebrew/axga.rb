class Axga < Formula
  desc "5.7MB AI coding agent — runs on 1GB VPS"
  homepage "https://github.com/KJ-AIML/axga-harness-agent-rs"
  version "0.1.0"
  license "MIT"

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/KJ-AIML/axga-harness-agent-rs/releases/download/v#{version}/axga-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256"
    elsif Hardware::CPU.arm?
      url "https://github.com/KJ-AIML/axga-harness-agent-rs/releases/download/v#{version}/axga-v#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256"
    end
  end

  on_macos do
    if Hardware::CPU.intel?
      url "https://github.com/KJ-AIML/axga-harness-agent-rs/releases/download/v#{version}/axga-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256"
    elsif Hardware::CPU.arm?
      url "https://github.com/KJ-AIML/axga-harness-agent-rs/releases/download/v#{version}/axga-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256"
    end
  end

  def install
    bin.install "axga"
  end

  test do
    system "#{bin}/axga", "--version"
  end
end
