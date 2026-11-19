require "fastlane/action"
require "fastlane/boolean"
require "fastlane_core/configuration/config_item"
require_relative "../helper/vitrine_helper"

module Fastlane
  module Actions
    module SharedValues
      VITRINE_IMPORTED_COUNT = :VITRINE_IMPORTED_COUNT
    end

    # `vitrine import screenshots`: copies the raw screenshots from
    # snapshot / screengrab (or any folder) into the Vitrine Studio project
    # (`screenshots/<lang>/<variant>/<key>.png`). Run it after
    # `capture_screenshots` / `capture_android_screenshots` and before
    # `vitrine_export`.
    class VitrineImportAction < Action
      UNMAPPED = %w[unresolved duplicate].freeze
      COPIES = %w[new overwrite].freeze

      def self.run(params)
        binary = Helper::VitrineHelper.binary(params[:binary])
        source = File.expand_path(params[:source] || default_source)
        UI.user_error!("Screenshot folder not found: #{source}") unless File.directory?(source)

        report, code = Helper::VitrineHelper.run_json(binary: binary, project: params[:project], args: cli_args(params, source), what: "import")
        print_plan(report)
        if code == 1
          UI.user_error!("Vitrine Studio import: some screenshots could not be mapped. Add lang_map / device_map / key_map " \
                         "(or run with dry_run: true to see the plan).")
        end

        Actions.lane_context[SharedValues::VITRINE_IMPORTED_COUNT] = params[:dry_run] ? summarize_dry_run(report) : summarize(report)
        report
      end

      def self.cli_args(params, source)
        args = ["import", "screenshots", source, "--format", params[:format], "--format-output", "json"]
        { lang_map: "--lang-map", device_map: "--device-map", key_map: "--key-map" }.each do |key, flag|
          (params[key] || {}).each { |k, v| args.push(flag, "#{k}=#{v}") }
        end
        args << "--include-framed" if params[:include_framed]
        args << "--save-mapping" if params[:save_mapping]
        args << "--dry-run" if params[:dry_run]
        args
      end

      def self.print_plan(report)
        plan = report["plan"] || {}
        print_unmapped(Array(plan["items"]))
        own = Array(plan["skipped"]).count { |s| s["reason"].to_s.include?("Vitrine Studio") }
        UI.message("Skipped #{own} image(s) exported by Vitrine Studio (not captures)") if own.positive?
        Array(report.dig("applied", "failed")).each { |src, err| UI.error("#{src}: #{err}") }
        Array(plan["missing"]).each do |m|
          UI.important("missing screenshot #{m['lang']}/#{m['variant']}/#{m['key']} (#{m['status']})")
        end
      end

      # Files that could not be mapped (unknown device/locale/screen) or
      # that map to the same destination twice.
      def self.print_unmapped(items)
        items.select { |i| UNMAPPED.include?(i["action"]) }.each do |item|
          hint = Array(item["notes"]).join("; ")
          UI.error("#{item['source']}: #{item['action']}#{hint.empty? ? '' : " – #{hint}"}")
        end
      end

      def self.summarize_dry_run(report)
        copies = Array(report.dig("plan", "items")).count { |i| COPIES.include?(i["action"]) }
        UI.success("Vitrine Studio import (dry run): #{copies} file(s) would be copied")
        0
      end

      def self.summarize(report)
        applied = report["applied"] || {}
        copied = Array(applied["copied"]).length
        UI.success("Vitrine Studio: #{copied} screenshot(s) imported, #{Array(applied['unchanged']).length} unchanged")
        UI.message("Mapping stored in project.yaml") if report["mapping_saved"]
        copied
      end

      # Raw captures belong in their own folder (`fastlane/captures`,
      # see the Snapfile / Screengrabfile snippets in the README), so
      # deliver/supply upload only Vitrine Studio's images. Falls back to
      # `./fastlane`; the import then skips Vitrine Studio's own exports.
      CAPTURES = "./fastlane/captures"

      def self.default_source
        return CAPTURES if File.directory?(CAPTURES)

        UI.important("No #{CAPTURES} folder: importing from ./fastlane. Point snapshot/screengrab's output_directory " \
                     "to fastlane/captures so the raw captures are not uploaded next to Vitrine Studio's images.")
        "./fastlane"
      end

      def self.description
        "Import raw screenshots (snapshot, screengrab or a folder) into a Vitrine Studio project"
      end

      def self.authors
        ["Vitrine Studio"]
      end

      def self.output
        [
          ["VITRINE_IMPORTED_COUNT", "Number of screenshots copied into the project (0 for a dry run)"]
        ]
      end

      def self.return_value
        "The import report (Hash: ok, dry_run, plan, applied, mapping_saved)"
      end

      def self.available_options
        [
          FastlaneCore::ConfigItem.new(key: :project, env_name: "VITRINE_PROJECT", description: "Project folder (with project.yaml)", default_value: "."),
          FastlaneCore::ConfigItem.new(key: :source, env_name: "VITRINE_IMPORT_SOURCE",
                                       description: "Folder with the raw screenshots (snapshot's screenshots/ + screengrab's metadata/android/ below it, or any folder). " \
                                                    "Default: ./fastlane/captures, else ./fastlane",
                                       optional: true),
          FastlaneCore::ConfigItem.new(key: :format, description: "Source layout: auto, fastlane, vitrine or flat", default_value: "auto",
                                       verify_block: proc { |v| UI.user_error!("format must be auto, fastlane, vitrine or flat") unless %w[auto fastlane vitrine flat].include?(v) }),
          FastlaneCore::ConfigItem.new(key: :lang_map, description: "Locale → language, e.g. { \"de-DE\" => \"de\" }", type: Hash, optional: true),
          FastlaneCore::ConfigItem.new(key: :device_map, description: "Device name → screenshot variant, e.g. { \"iPhone 17 Pro Max\" => \"ios-phone\" }", type: Hash, optional: true),
          FastlaneCore::ConfigItem.new(key: :key_map, description: "Screen name → screenshot key, e.g. { \"01Home\" => \"home\" }", type: Hash, optional: true),
          FastlaneCore::ConfigItem.new(key: :include_framed, description: "Also import frameit's *_framed.png files (skipped by default)", type: Boolean, default_value: false),
          FastlaneCore::ConfigItem.new(key: :save_mapping, description: "Store the effective mapping in project.yaml so later imports map the same way", type: Boolean, default_value: false),
          FastlaneCore::ConfigItem.new(key: :dry_run, description: "Only show the plan; copy nothing", type: Boolean, default_value: false),
          FastlaneCore::ConfigItem.new(key: :binary, env_name: "VITRINE_CLI", description: "Path of the vitrine binary", optional: true)
        ]
      end

      def self.is_supported?(platform)
        %i[ios mac android].include?(platform)
      end

      def self.example_code
        [
          '# Snapfile: output_directory("./fastlane/captures/screenshots")
          capture_screenshots
          vitrine_import(project: "store-screens")   # reads ./fastlane/captures
          vitrine_export(project: "store-screens")',
          'vitrine_import(project: "store-screens",
                         device_map: { "iPhone 17 Pro Max" => "ios-phone" }, save_mapping: true)'
        ]
      end

      def self.category
        :screenshots
      end
    end
  end
end
