require "fastlane/action"
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
      def self.run(params)
        binary = Helper::VitrineHelper.binary(params[:binary])
        source = File.expand_path(params[:source])
        UI.user_error!("Screenshot folder not found: #{source}") unless File.directory?(source)

        args = ["import", "screenshots", source, "--format", params[:format], "--format-output", "json"]
        (params[:lang_map] || {}).each { |k, v| args.push("--lang-map", "#{k}=#{v}") }
        (params[:device_map] || {}).each { |k, v| args.push("--device-map", "#{k}=#{v}") }
        (params[:key_map] || {}).each { |k, v| args.push("--key-map", "#{k}=#{v}") }
        args << "--include-framed" if params[:include_framed]
        args << "--save-mapping" if params[:save_mapping]
        args << "--dry-run" if params[:dry_run]

        stdout, code = Helper::VitrineHelper.run(binary: binary, project: params[:project], args: args)
        UI.user_error!("vitrine import could not run (exit #{code})") if code == 2
        report = Helper::VitrineHelper.parse_json(stdout, "import")
        plan = report["plan"] || {}

        # Files that could not be mapped (unknown device/locale/screen) or
        # that map to the same destination twice.
        Array(plan["items"]).each do |item|
          next unless %w[unresolved duplicate].include?(item["action"])

          hint = Array(item["notes"]).join("; ")
          UI.error("#{item['source']}: #{item['action']}#{hint.empty? ? '' : " – #{hint}"}")
        end
        Array(report.dig("applied", "failed")).each { |src, err| UI.error("#{src}: #{err}") }
        Array(plan["missing"]).each do |m|
          UI.important("missing screenshot #{m['lang']}/#{m['variant']}/#{m['key']} (#{m['status']})")
        end

        if code == 1
          UI.user_error!("Vitrine Studio import: some screenshots could not be mapped. Add lang_map / device_map / key_map " \
                         "(or run with dry_run: true to see the plan).")
        end

        if params[:dry_run]
          copies = Array(plan["items"]).count { |i| %w[new overwrite].include?(i["action"]) }
          UI.success("Vitrine Studio import (dry run): #{copies} file(s) would be copied")
          Actions.lane_context[SharedValues::VITRINE_IMPORTED_COUNT] = 0
        else
          applied = report["applied"] || {}
          copied = Array(applied["copied"]).length
          unchanged = Array(applied["unchanged"]).length
          UI.success("Vitrine Studio: #{copied} screenshot(s) imported, #{unchanged} unchanged")
          UI.message("Mapping stored in project.yaml") if report["mapping_saved"]
          Actions.lane_context[SharedValues::VITRINE_IMPORTED_COUNT] = copied
        end
        report
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
                                       description: "Folder with the raw screenshots: the fastlane folder (snapshot's screenshots/ + screengrab's metadata/android/), or any folder",
                                       default_value: "./fastlane"),
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
          'capture_screenshots
          vitrine_import(project: "store-screens")
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
