describe Fastlane::Actions::VitrineExportAction do
  let(:action) { described_class }
  let(:file) { { "path" => "screenshots/en-US/01_ios-iphone-69.png", "target" => "ios-iphone-69", "lang" => "en", "skipped" => false } }

  around do |example|
    saved = %w[DELIVER_SCREENSHOTS_PATH SUPPLY_METADATA_PATH].to_h { |k| [k, ENV.fetch(k, nil)] }
    saved.each_key { |k| ENV.delete(k) }
    example.run
  ensure
    saved.each { |k, v| ENV[k] = v }
  end

  it "exports in the fastlane layout and sets the lane context" do
    Dir.mktmpdir do |out|
      FileUtils.mkdir_p(File.join(out, "screenshots"))
      report = { "out_dir" => out, "layout" => "fastlane", "files" => [file], "issues" => [], "errors" => [] }
      run = run_with_fake_vitrine(action, { project: "store", output_directory: out, foreign_images: "remove", languages: ["en"] }, stdout: report)
      expect(run[:error]).to be_nil
      expect(run[:args]).to eq(["--project", "store", "export", "--layout", "fastlane", "--out", out, "--format", "json", "--no-html",
                                "--remove-foreign", "--lang", "en"])
      screenshots = File.join(out, "screenshots")
      expect(Fastlane::Actions.lane_context[Fastlane::Actions::SharedValues::VITRINE_SCREENSHOTS_PATH]).to eq(screenshots)
      expect(ENV.fetch("DELIVER_SCREENSHOTS_PATH", nil)).to eq(screenshots)
      expect(ENV.fetch("SUPPLY_METADATA_PATH", nil)).to be_nil # folder does not exist
    end
  end

  it "stops the lane on foreign images by default" do
    report = { "out_dir" => "x", "files" => [file], "issues" => [], "errors" => [], "foreign" => ["screenshots/en-US/raw.png"] }
    run = run_with_fake_vitrine(action, {}, stdout: report)
    expect(run[:error].message).to include("1 image(s) in the upload folders were not exported by Vitrine Studio")
  end

  it "only warns about foreign images with foreign_images: warn" do
    report = { "out_dir" => "x", "files" => [file], "issues" => [], "errors" => [], "foreign" => ["screenshots/en-US/raw.png"] }
    run = run_with_fake_vitrine(action, { foreign_images: "warn" }, stdout: report)
    expect(run[:error]).to be_nil
  end

  it "does not pass --remove-foreign for the standard layout" do
    report = { "out_dir" => "x", "files" => [file], "issues" => [], "errors" => [] }
    run = run_with_fake_vitrine(action, { layout: "standard", foreign_images: "remove", force: true }, stdout: report)
    expect(run[:args]).not_to include("--remove-foreign")
    expect(run[:args]).to include("--force")
  end

  it "fails the lane when the export has errors (exit 1)" do
    report = { "out_dir" => "x", "files" => [], "issues" => [], "errors" => ["boom"] }
    run = run_with_fake_vitrine(action, {}, stdout: report, exit_code: 1)
    expect(run[:error].message).to include("export failed")
  end

  it "rejects an unknown layout" do
    expect { FastlaneCore::Configuration.create(action.available_options, { layout: "flat" }) }.to raise_error(/layout must be/)
  end
end
