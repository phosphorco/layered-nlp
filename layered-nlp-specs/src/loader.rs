//! Fixture file loading.

use crate::{parse_fixture, NlpFixture, SpecError};
use std::fs;
use std::path::Path;

/// Load a single fixture file.
pub fn load_fixture(path: &Path) -> Result<NlpFixture, SpecError> {
    let content = fs::read_to_string(path)
        .map_err(|e| SpecError::Load {
            path: path.display().to_string(),
            message: e.to_string(),
        })?;
    parse_fixture(&content)
}

/// Load all fixtures from a directory (glob: **/*.nlp).
pub fn load_all_fixtures(dir: &Path) -> Result<Vec<(String, NlpFixture)>, SpecError> {
    let mut fixtures = Vec::new();
    load_fixtures_recursive(dir, dir, &mut fixtures)?;
    Ok(fixtures)
}

fn normalize_fixture_relpath(base: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(base).unwrap_or(path);
    relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn load_fixtures_recursive(
    base: &Path,
    dir: &Path,
    fixtures: &mut Vec<(String, NlpFixture)>,
) -> Result<(), SpecError> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(dir).map_err(|e| SpecError::Load {
        path: dir.display().to_string(),
        message: e.to_string(),
    })? {
        let entry = entry.map_err(|e| SpecError::Load {
            path: dir.display().to_string(),
            message: e.to_string(),
        })?;
        let path = entry.path();

        if path.is_dir() {
            load_fixtures_recursive(base, &path, fixtures)?;
        } else if path.extension().map_or(false, |e| e == "nlp") {
            let relative = normalize_fixture_relpath(base, &path);
            let fixture = load_fixture(&path)?;
            fixtures.push((relative, fixture));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_fixture() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join("line")
            .join("obligations")
            .join("modal")
            .join("simple-obligation.nlp");
        let fixture = load_fixture(&path).unwrap();
        assert!(fixture.title.is_some());
    }

    #[test]
    fn test_load_all_fixtures() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
        let fixtures = load_all_fixtures(&dir).unwrap();
        assert!(fixtures.len() >= 3); // We have at least 3 fixtures
    }
}
