$LOAD_PATH.unshift(File.expand_path("../lib", __dir__))

require "simplecov"

SimpleCov.start

# This module is only used to check the environment is currently a testing env
module SpecHelper
  FAKE_VITRINE = File.expand_path("fixtures/bin/vitrine", __dir__)
end

require "fastlane" # to import the Action super class
require "fastlane/plugin/vitrine" # import the actual plugin

Fastlane.load_actions # load other actions (in case your plugin calls other actions or shared values)

require "json"
require "tmpdir"

# Runs an action with the fake CLI (spec/fixtures/bin/vitrine): it prints
# `stdout`, exits with `exit_code` and records its arguments.
def run_with_fake_vitrine(action, params, stdout:, exit_code: 0)
  Dir.mktmpdir do |dir|
    out_file = File.join(dir, "stdout")
    args_file = File.join(dir, "args.json")
    File.write(out_file, stdout.kind_of?(String) ? stdout : JSON.generate(stdout))
    env = { "FAKE_VITRINE_STDOUT" => out_file, "FAKE_VITRINE_EXIT" => exit_code.to_s, "FAKE_VITRINE_ARGS" => args_file }
    saved = env.keys.to_h { |k| [k, ENV.fetch(k, nil)] }
    env.each { |k, v| ENV[k] = v }
    begin
      config = FastlaneCore::Configuration.create(action.available_options, { binary: SpecHelper::FAKE_VITRINE }.merge(params))
      result = nil
      error = nil
      begin
        result = action.run(config)
      rescue FastlaneCore::Interface::FastlaneError => e
        error = e
      end
      args = File.exist?(args_file) ? JSON.parse(File.read(args_file)) : nil
      return { result: result, error: error, args: args }
    ensure
      saved.each { |k, v| ENV[k] = v }
    end
  end
end
