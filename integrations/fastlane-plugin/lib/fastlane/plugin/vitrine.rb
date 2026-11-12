require "fastlane/plugin/vitrine/version"

module Fastlane
  module Vitrine
    # Loads the actions and helpers (fastlane's plugin convention).
    def self.all_classes
      Dir[File.expand_path("**/{actions,helper}/*.rb", File.dirname(__FILE__))]
    end
  end
end

Fastlane::Vitrine.all_classes.each do |current|
  require current
end
