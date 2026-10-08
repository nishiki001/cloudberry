cask "cloudberry" do
  version "0.1.0"
  sha256 "SKIP_UNTIL_FIRST_RELEASE"

  url "https://github.com/nishiki001/cloudberry/releases/download/v#{version}/Cloudberry-macos-universal.dmg"
  name "Cloudberry"
  desc "Unofficial desktop client for YouTube Music (not affiliated with Google)"
  homepage "https://github.com/nishiki001/cloudberry"

  depends_on macos: ">= :big_sur"

  app "Cloudberry.app"

  zap trash: [
    "~/Library/Application Support/cloudberry",
    "~/Library/Caches/cloudberry",
    "~/Library/Preferences/cloudberry",
  ]
end
