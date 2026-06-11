class Axga < Formula
  desc "5.7MB AI coding agent — runs on 1GB VPS"
  homepage "https://github.com/KJ-AIML/axga-harness-agent-rs"
  version "0.1.0"
  license "MIT"

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/KJ-AIML/axga-harness-agent-rs/releases/download/v#{version}/axga-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "c751933392de5941583d8a9ec257ba0fe865740c819f5c36b18c07e7e4f6e6d8"
    elsif Hardware::CPU.arm?
      url "https://github.com/KJ-AIML/axga-harness-agent-rs/releases/download/v#{version}/axga-v#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "# FILL at release time with scripts/sha256-update.sh"
    end
  end

  on_macos do
    if Hardware::CPU.intel?
      url "https://github.com/KJ-AIML/axga-harness-agent-rs/releases/download/v#{version}/axga-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "# FILL at release time with scripts/sha256-update.sh"
    elsif Hardware::CPU.arm?
      url "https://github.com/KJ-AIML/axga-harness-agent-rs/releases/download/v#{version}/axga-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "# FILL at release time with scripts/sha256-update.sh"
    end
  end

  def install
    bin.install "axga"
  end

  test do
    system "#{bin}/axga", "--version"
  end
end
