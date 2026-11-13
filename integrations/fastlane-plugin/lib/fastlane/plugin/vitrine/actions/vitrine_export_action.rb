require "fastlane/action"
require_relative "../helper/vitrine_helper"

module Fastlane
  module Actions
    module SharedValues
      VITRINE_EXPORT_DIR = :VITRINE_EXPORT_DIR
      VITRINE_SCREENSHOTS_PATH = :VITRINE_SCREENSHOTS_PATH
      VITRINE_METADATA_PATH = :VITRINE_METADATA_PATH
    end

    # `vitrine export --layout fastlane`: writes deliver's
    # `screenshots/<locale>/` and supply's `metadata/android/<locale>/images/`
    # folders, so `upload_to_app_store` / `upload_to_play_store` upload them
    # unchanged. Replaces `frameit`.
    class VitrineExportAction < Action
      def self.run(params)
        binary = Helper::VitrineHelper.binary(params[:binary])
        out = File.expand_path(params[:output_directory])
        args = ["export", "--layout", params[:layout], "--out", out, "--format", "json", "--no-html"]
        args << "--force" if params[:force]
        fastlane_layout = params[:layout] == "fastlane"
        args << "--remove-foreign" if fastlane_layout && params[:foreign_images] == "remove"
        Array(params[:targets]).each { |t| args.push("--target", t) }
        Array(params[:languages]).each { |l| args.push("--lang", l) }
        stdout, code = Helper::VitrineHelper.run(binary: binary, project: params[:project], args: args)
        UI.user_error!("vitrine export could not run (exit #{code})") if code == 2
        report = Helper::VitrineHelper.parse_json(stdout, "export")
        Array(report["notes"]).each { |n| UI.important("#{n['target']} #{n['lang']}: #{n['message']}") }
        Array(report["errors"]).each { |e| UI.error(e.to_s) }
        UI.user_error!("Vitrine Studio export failed") if code == 1

        Array(report["removed_foreign"]).each { |f| UI.message("Removed (not from Vitrine Studio): #{f}") }
        # Images deliver/supply would upload together with Vitrine Studio's:
        # raw snapshot/screengrab captures, frameit output.
        foreign = Array(report["foreign"])
        if foreign.any? && params[:foreign_images] == "fail"
          foreign.first(10).each { |f| UI.error("not from Vitrine Studio: #{File.join(out, f)}") }
          UI.user_error!("#{foreign.length} image(s) in the upload folders were not exported by Vitrine Studio; " \
                         "deliver/supply would upload them too. Let snapshot/screengrab write to fastlane/captures " \
                         "(output_directory) and import from there, or use foreign_images: \"remove\".")
        end

        written = report["files"].count { |f| !f["skipped"] }
        UI.success("Vitrine Studio: #{written} file(s) written, #{report['files'].length - written} unchanged → #{out}")
        Actions.lane_context[SharedValues::VITRINE_EXPORT_DIR] = out
        if fastlane_layout
          screenshots = File.join(out, "screenshots")
          metadata = File.join(out, "metadata", "android")
          Actions.lane_context[SharedValues::VITRINE_SCREENSHOTS_PATH] = screenshots
          Actions.lane_context[SharedValues::VITRINE_METADATA_PATH] = metadata
          # deliver / supply read these when no explicit path is given.
          ENV["DELIVER_SCREENSHOTS_PATH"] ||= screenshots if File.directory?(screenshots)
          ENV["SUPPLY_METADATA_PATH"] ||= metadata if File.directory?(metadata)
        end
        report
      end

      def self.description
        "Export App Store / Play Store screenshots with Vitrine Studio (fastlane folder layout by default)"
      end

      def self.authors
        ["Vitrine Studio"]
      end

      def self.output
        [
          ["VITRINE_EXPORT_DIR", "The export folder"],
          ["VITRINE_SCREENSHOTS_PATH", "deliver's screenshots folder (layout fastlane)"],
          ["VITRINE_METADATA_PATH", "supply's metadata folder (layout fastlane)"]
        ]
      end

      def self.return_value
        "The export report (Hash: out_dir, files, issues, notes, errors)"
      end

      def self.available_options
        [
          FastlaneCore::ConfigItem.new(key: :project, env_name: "VITRINE_PROJECT", description: "Project folder (with project.yaml)", default_value: "."),
          FastlaneCore::ConfigItem.new(key: :output_directory, env_name: "VITRINE_EXPORT_DIR", description: "Export folder (fastlane layout: the folder that gets screenshots/ and metadata/android/)", default_value: "./fastlane"),
          FastlaneCore::ConfigItem.new(key: :layout, description: "fastlane or standard", default_value: "fastlane",
                                       verify_block: proc { |v| UI.user_error!("layout must be fastlane or standard") unless %w[fastlane standard].include?(v) }),
          FastlaneCore::ConfigItem.new(key: :targets, description: "Only these targets", type: Array, optional: true),
          FastlaneCore::ConfigItem.new(key: :languages, description: "Only these languages", type: Array, optional: true),
          FastlaneCore::ConfigItem.new(key: :foreign_images,
                                       description: "Images in the deliver/supply folders that Vitrine Studio did not export (raw captures, frameit output): " \
                                                    "fail (default: stop the lane), warn, or remove (delete them after the export)",
                                       default_value: "fail",
                                       verify_block: proc { |v| UI.user_error!("foreign_images must be fail, warn or remove") unless %w[fail warn remove].include?(v) }),
          FastlaneCore::ConfigItem.new(key: :force,description: "Render everything (ignore the export cache)", type: Boolean, default_value: false),
          FastlaneCore::ConfigItem.new(key: :binary, env_name: "VITRINE_CLI", description: "Path of the vitrine binary", optional: true)
        ]
      end

      def self.is_supported?(platform)
        %i[ios mac android].include?(platform)
      end

      def self.example_code
        [
          'vitrine_export(project: "store-screens")
          upload_to_app_store(skip_binary_upload: true, skip_metadata: true, overwrite_screenshots: true)'
        ]
      end

      def self.category
        :screenshots
      end
    end
  end
end
