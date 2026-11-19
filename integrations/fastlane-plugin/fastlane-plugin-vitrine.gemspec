lib = File.expand_path("lib", __dir__)
$LOAD_PATH.unshift(lib) unless $LOAD_PATH.include?(lib)
require "fastlane/plugin/vitrine/version"

Gem::Specification.new do |spec|
  spec.name          = "fastlane-plugin-vitrine"
  spec.version       = Fastlane::Vitrine::VERSION
  spec.author        = "Vitrine Studio"
  spec.summary       = "Import, validate and export App Store / Play Store screenshots with Vitrine Studio (like frameit)"
  spec.homepage      = "https://github.com/vitrinestudio-app/fastlane-plugin-vitrine"
  # Owner decision 2026-11-12: integrations that only call the CLI as a
  # separate program are MIT (the app/CLI stay GPL-3.0-or-later, ADR 0022).
  spec.license       = "MIT"
  spec.metadata      = {
    "homepage_uri" => "https://vitrinestudio.app",
    "source_code_uri" => "https://github.com/vitrinestudio-app/fastlane-plugin-vitrine",
    "documentation_uri" => "https://vitrinestudio.app/docs/",
    "rubygems_mfa_required" => "true"
  }

  spec.files         = Dir["lib/**/*"] + %w[README.md LICENSE]
  spec.require_paths = ["lib"]
  spec.required_ruby_version = ">= 3.1"
  # The plugin only calls the `vitrine` binary; fastlane itself is the host
  # (no dependency on fastlane, as fastlane's plugin template says).
end
