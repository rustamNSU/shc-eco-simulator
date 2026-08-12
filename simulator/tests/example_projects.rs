use std::{fs, path::PathBuf};

use simulator::ProjectFile;

#[test]
fn bundled_example_projects_decode_successfully() {
    let examples_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples");
    let mut project_count = 0;

    for entry in fs::read_dir(&examples_dir).expect("examples directory should be readable") {
        let path = entry
            .expect("example directory entry should be readable")
            .path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }

        let json = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("could not read example {}: {error}", path.display()));
        let project: ProjectFile = serde_json::from_str(&json)
            .unwrap_or_else(|error| panic!("could not decode example {}: {error}", path.display()));
        assert!(
            matches!(project.version, 1 | simulator::PROJECT_FORMAT_VERSION),
            "example {} uses unsupported project version {}",
            path.display(),
            project.version
        );
        assert!(
            !project.buildings.is_empty(),
            "example {} should contain buildings",
            path.display()
        );
        project_count += 1;
    }

    assert!(
        project_count > 0,
        "at least one example project is required"
    );
}
