# DRAFT – not published to RubyGems. See README.md.
lib = File.expand_path("lib", __dir__)
$LOAD_PATH.unshift(lib) unless $LOAD_PATH.include?(lib)
require "fastlane/plugin/vitrine/version"

Gem::Specification.new do |spec|
  spec.name          = "fastlane-plugin-vitrine"
  spec.version       = Fastlane::Vitrine::VERSION
  spec.author        = "Vitrine Studio"
  spec.summary       = "Validate and export App Store / Play Store screenshots with Vitrine Studio (replaces frameit)"
  spec.homepage      = "https://vitrinestudio.app"
  # Owner decision 2026-11-12: integrations that only call the CLI as a
  # separate program are MIT (the app/CLI stay GPL-3.0-or-later, ADR 0022).
  spec.license       = "MIT"
  spec.metadata      = {
    "source_code_uri" => "https://github.com/vitrinestudio-app/vitrine/tree/main/integrations/fastlane-plugin",
    "rubygems_mfa_required" => "true"
  }

  spec.files         = Dir["lib/**/*"] + %w[README.md LICENSE]
  spec.require_paths = ["lib"]
  spec.required_ruby_version = ">= 2.6"
  # The plugin only calls the `vitrine` binary; fastlane itself is the host.
end
