describe Fastlane::Actions::VitrineImportAction do
  let(:action) { described_class }

  it "passes the maps and flags to `vitrine import screenshots`" do
    Dir.mktmpdir do |source|
      report = { "ok" => true, "dry_run" => true, "plan" => { "items" => [{ "source" => "a.png", "action" => "new" }], "skipped" => [], "missing" => [] } }
      params = { project: "store", source: source, lang_map: { "de-DE" => "de" }, device_map: { "iPhone 17 Pro Max" => "ios-phone" },
                 key_map: { "01Home" => "home" }, include_framed: true, save_mapping: true, dry_run: true }
      run = run_with_fake_vitrine(action, params, stdout: report)
      expect(run[:error]).to be_nil
      expect(run[:args]).to eq(["--project", "store", "import", "screenshots", source, "--format", "auto", "--format-output", "json",
                                "--lang-map", "de-DE=de", "--device-map", "iPhone 17 Pro Max=ios-phone", "--key-map", "01Home=home",
                                "--include-framed", "--save-mapping", "--dry-run"])
      expect(Fastlane::Actions.lane_context[Fastlane::Actions::SharedValues::VITRINE_IMPORTED_COUNT]).to eq(0)
    end
  end

  it "counts the copied files" do
    Dir.mktmpdir do |source|
      report = { "ok" => true, "dry_run" => false, "plan" => { "items" => [] }, "applied" => { "copied" => %w[a b], "unchanged" => ["c"] } }
      run = run_with_fake_vitrine(action, { source: source }, stdout: report)
      expect(run[:error]).to be_nil
      expect(Fastlane::Actions.lane_context[Fastlane::Actions::SharedValues::VITRINE_IMPORTED_COUNT]).to eq(2)
    end
  end

  it "fails the lane when files cannot be mapped (exit 1)" do
    Dir.mktmpdir do |source|
      report = { "ok" => false, "plan" => { "items" => [{ "source" => "x.png", "action" => "unresolved", "notes" => ["locale `de-DE` matches no project language"] }] } }
      run = run_with_fake_vitrine(action, { source: source }, stdout: report, exit_code: 1)
      expect(run[:error].message).to include("could not be mapped")
    end
  end

  it "fails when the source folder does not exist" do
    run = run_with_fake_vitrine(action, { source: "/nonexistent/captures" }, stdout: "{}")
    expect(run[:error].message).to include("Screenshot folder not found")
    expect(run[:args]).to be_nil
  end
end
