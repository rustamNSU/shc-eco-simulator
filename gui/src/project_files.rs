use std::{
    env, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use simulator::ProjectFile;

const MAX_RECENT_FILES: usize = 10;

#[derive(Debug, Default, Serialize, Deserialize)]
struct RecentFiles {
    paths: Vec<PathBuf>,
}

pub struct ProjectSession {
    app_data_dir: PathBuf,
    current_path: Option<PathBuf>,
    recent: RecentFiles,
}

impl ProjectSession {
    pub fn new() -> Self {
        let app_data_dir = app_data_dir();
        let recent = fs::read_to_string(app_data_dir.join("recent.json"))
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();

        Self {
            app_data_dir,
            current_path: None,
            recent,
        }
    }

    pub fn current_name(&self) -> String {
        self.current_path
            .as_deref()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_string())
    }

    pub fn latest_recent_name(&self) -> String {
        self.latest_recent_path()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    pub fn save(&mut self, project: &ProjectFile) -> Result<Option<PathBuf>, String> {
        let Some(path) = self.current_path.clone() else {
            return self.save_as(project);
        };
        self.write_project(&path, project)?;
        Ok(Some(path))
    }

    pub fn save_as(&mut self, project: &ProjectFile) -> Result<Option<PathBuf>, String> {
        let default_dir = self.default_dialog_dir();
        fs::create_dir_all(&default_dir)
            .map_err(|error| format!("could not create project folder: {error}"))?;

        let mut dialog = rfd::FileDialog::new()
            .add_filter("SHC Eco project", &["json"])
            .set_directory(default_dir);
        dialog = if let Some(current) = &self.current_path {
            dialog.set_file_name(current.file_name().unwrap_or_default().to_string_lossy())
        } else {
            dialog.set_file_name("eco.json")
        };

        let Some(mut path) = dialog.save_file() else {
            return Ok(None);
        };
        if path.extension().is_none() {
            path.set_extension("json");
        }
        self.write_project(&path, project)?;
        Ok(Some(path))
    }

    pub fn open_dialog(&mut self) -> Result<Option<(ProjectFile, PathBuf)>, String> {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("SHC Eco project", &["json"])
            .set_directory(self.default_dialog_dir())
            .pick_file()
        else {
            return Ok(None);
        };

        self.read_project(path).map(Some)
    }

    pub fn open_latest_recent(&mut self) -> Result<Option<(ProjectFile, PathBuf)>, String> {
        let Some(path) = self.latest_recent_path().map(Path::to_path_buf) else {
            return Ok(None);
        };
        self.read_project(path).map(Some)
    }

    pub fn mark_opened(&mut self, path: PathBuf) -> Result<(), String> {
        self.current_path = Some(path.clone());
        self.record_recent(path)
    }

    fn write_project(&mut self, path: &Path, project: &ProjectFile) -> Result<(), String> {
        let json = serde_json::to_string_pretty(project)
            .map_err(|error| format!("could not encode project: {error}"))?;
        fs::write(path, json).map_err(|error| format!("could not save project: {error}"))?;
        self.mark_opened(path.to_path_buf())
    }

    fn read_project(&self, path: PathBuf) -> Result<(ProjectFile, PathBuf), String> {
        let json = fs::read_to_string(&path)
            .map_err(|error| format!("could not open project: {error}"))?;
        let project = serde_json::from_str(&json)
            .map_err(|error| format!("invalid project JSON: {error}"))?;
        Ok((project, path))
    }

    fn record_recent(&mut self, path: PathBuf) -> Result<(), String> {
        self.recent.paths.retain(|existing| existing != &path);
        self.recent.paths.insert(0, path);
        self.recent.paths.truncate(MAX_RECENT_FILES);
        fs::create_dir_all(&self.app_data_dir)
            .map_err(|error| format!("could not create app-data folder: {error}"))?;
        let json = serde_json::to_string_pretty(&self.recent)
            .map_err(|error| format!("could not encode recent files: {error}"))?;
        fs::write(self.app_data_dir.join("recent.json"), json)
            .map_err(|error| format!("could not save recent files: {error}"))
    }

    fn default_dialog_dir(&self) -> PathBuf {
        self.current_path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.app_data_dir.join("Projects"))
    }

    fn latest_recent_path(&self) -> Option<&Path> {
        self.recent
            .paths
            .iter()
            .find(|path| path.is_file())
            .map(PathBuf::as_path)
    }
}

fn app_data_dir() -> PathBuf {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("SHCEcoSimulator")
}

#[cfg(test)]
mod tests {
    use super::ProjectSession;

    #[test]
    fn new_session_has_a_safe_display_name() {
        assert_eq!(ProjectSession::new().current_name(), "Untitled");
    }
}
