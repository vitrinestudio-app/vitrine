describe Fastlane::Actions::VitrineValidateAction do
  let(:action) { described_class }
  let(:finding) { { "level" => "warn", "code" => "screenshot_missing", "target" => "ios-iphone-69", "lang" => "en", "message" => "missing" } }

  it "passes the options to `vitrine validate` and returns the report" do
    report = { "ok" => true, "errors" => 0, "warnings" => 1, "infos" => 0, "findings" => [finding] }
    run = run_with_fake_vitrine(action, { project: "store", strict: true, preview: true, targets: ["ios-iphone-69"], languages: %w[en de] }, stdout: report)
    expect(run[:error]).to be_nil
    expect(run[:result]).to eq(report)
    expect(run[:args]).to eq(%w[--project store validate --format json --strict --preview --target ios-iphone-69 --lang en --lang de])
  end

  it "fails the lane on project problems (exit 1)" do
    report = { "ok" => false, "errors" => 1, "warnings" => 0, "infos" => 0, "findings" => [finding.merge("level" => "error")] }
    run = run_with_fake_vitrine(action, {}, stdout: report, exit_code: 1)
    expect(run[:error].message).to include("validation failed: 1 error(s)")
  end

  it "fails the lane when the CLI could not run (exit 2)" do
    run = run_with_fake_vitrine(action, {}, stdout: "", exit_code: 2)
    expect(run[:error].message).to include("could not run (exit 2)")
  end

  it "fails on output that is not JSON" do
    run = run_with_fake_vitrine(action, {}, stdout: "oops")
    expect(run[:error].message).to include("did not return JSON")
  end
end
