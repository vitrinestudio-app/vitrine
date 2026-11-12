require "json"
require "open3"

module Fastlane
  module Helper
    # Finds and runs the `vitrine` CLI. Everything the plugin does goes
    # through the CLI; the plugin never reads or writes project files.
    class VitrineHelper
      # `binary` option → $VITRINE_CLI → `vitrine` on PATH.
      def self.binary(explicit = nil)
        candidate = explicit || ENV["VITRINE_CLI"]
        return candidate if candidate && !candidate.empty?

        exe = Gem.win_platform? ? "vitrine.exe" : "vitrine"
        found = ENV.fetch("PATH", "").split(File::PATH_SEPARATOR).map { |d| File.join(d, exe) }.find { |p| File.executable?(p) }
        return found if found

        UI.user_error!("The `vitrine` command was not found. Install Vitrine Studio (app settings → Install command line tool), " \
                       "download the CLI archive from the GitHub release, or set the `binary` option / VITRINE_CLI.")
      end

      # Runs `vitrine --project <project> <args…>`; returns [stdout, exit code].
      # Exit codes: 0 ok, 1 project problems, 2 could not run.
      def self.run(binary:, project:, args:, print_command: true)
        cmd = [binary, "--project", project] + args
        UI.command(cmd.join(" ")) if print_command
        stdout, stderr, status = Open3.capture3(*cmd)
        stderr.each_line { |l| UI.message(l.chomp) } unless stderr.empty?
        [stdout, status.exitstatus]
      end

      def self.parse_json(text, what)
        JSON.parse(text)
      rescue JSON::ParserError
        UI.user_error!("vitrine #{what} did not return JSON")
      end
    end
  end
end
