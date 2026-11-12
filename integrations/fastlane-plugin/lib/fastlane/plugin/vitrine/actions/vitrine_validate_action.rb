require "fastlane/action"
require_relative "../helper/vitrine_helper"

module Fastlane
  module Actions
    # `vitrine validate`: fails the lane on errors (and on warnings with
    # `strict`). Returns the report hash (ok, errors, warnings, findings).
    class VitrineValidateAction < Action
      def self.run(params)
        binary = Helper::VitrineHelper.binary(params[:binary])
        args = %w[validate --format json]
        args << "--strict" if params[:strict]
        args << "--preview" if params[:preview]
        Array(params[:targets]).each { |t| args.push("--target", t) }
        Array(params[:languages]).each { |l| args.push("--lang", l) }
        out, code = Helper::VitrineHelper.run(binary: binary, project: params[:project], args: args)
        UI.user_error!("vitrine validate could not run (exit #{code})") if code == 2
        report = Helper::VitrineHelper.parse_json(out, "validate")
        report["findings"].each do |f|
          scope = [f["target"], f["lang"] && "(#{f['lang']})"].compact.join(" ")
          line = "#{f['code']} #{scope}: #{f['message']}"
          case f["level"]
          when "error" then UI.error(line)
          when "warn" then UI.important(line)
          else UI.verbose(line)
          end
        end
        summary = "#{report['errors']} error(s), #{report['warnings']} warning(s)"
        UI.user_error!("Vitrine Studio validation failed: #{summary}") if code == 1
        UI.success("Vitrine Studio validation: #{summary}")
        report
      end

      def self.description
        "Validate a Vitrine Studio project (texts, screenshots, store rules)"
      end

      def self.authors
        ["Vitrine Studio"]
      end

      def self.return_value
        "The validation report (Hash: ok, errors, warnings, infos, findings)"
      end

      def self.available_options
        [
          FastlaneCore::ConfigItem.new(key: :project, env_name: "VITRINE_PROJECT", description: "Project folder (with project.yaml)", default_value: "."),
          FastlaneCore::ConfigItem.new(key: :strict, description: "Fail on warnings too", type: Boolean, default_value: false),
          FastlaneCore::ConfigItem.new(key: :preview, description: "Missing screenshots are warnings", type: Boolean, default_value: false),
          FastlaneCore::ConfigItem.new(key: :targets, description: "Only these targets", type: Array, optional: true),
          FastlaneCore::ConfigItem.new(key: :languages, description: "Only these languages", type: Array, optional: true),
          FastlaneCore::ConfigItem.new(key: :binary, env_name: "VITRINE_CLI", description: "Path of the vitrine binary", optional: true)
        ]
      end

      def self.is_supported?(platform)
        %i[ios mac android].include?(platform)
      end

      def self.category
        :screenshots
      end
    end
  end
end
