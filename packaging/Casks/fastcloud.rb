cask "fastcloud" do
  version "0.1.0"
  sha256 :no_check

  url "https://github.com/COMF2222/veirune/releases/latest/download/fastcloud-v#{version}-macos-arm64.tar.gz"
  name "Fastcloud"
  desc "Native SoundCloud client"
  homepage "https://github.com/COMF2222/veirune"

  app "Fastcloud.app"

  zap delete: [
    "~/Library/Application Support/fastcloud",
    "~/Library/Caches/fastcloud",
  ]
end
