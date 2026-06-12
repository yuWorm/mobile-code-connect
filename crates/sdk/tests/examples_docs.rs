use std::path::Path;

#[test]
fn sdk_workflow_examples_are_present_and_documented() {
    assert!(Path::new("examples/sdk_mock_workflow.rs").is_file());
    assert!(Path::new("examples/sdk_live_workflow.rs").is_file());

    let readme = std::fs::read_to_string("../../README.md").expect("read README");
    assert!(readme.contains("cargo run -p mobilecode_connect_sdk --example sdk_mock_workflow"));
    assert!(readme.contains("cargo run -p mobilecode_connect_sdk --example sdk_live_workflow"));
}
