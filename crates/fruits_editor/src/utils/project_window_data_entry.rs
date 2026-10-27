use std::ffi::OsString;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProjectWindowDataEntry {
    pub name: String,
    pub path: OsString,
    pub children: Vec<ProjectWindowDataEntry>,
}

impl ProjectWindowDataEntry {
    pub fn scan(src: &std::path::PathBuf) -> Self {
        Self {
            name: src.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default(),
            path: src.as_os_str().to_os_string(),
            children: Self::get_children(src),
        }
    }

    fn from_dir_entry(src: &std::fs::DirEntry) -> Self {
        let path = src.path();
        let is_dir = src.file_type().is_ok_and(|t| t.is_dir());

        Self {
            name: src.file_name().to_string_lossy().into_owned(),
            children: if is_dir { Self::get_children(&path) } else { Vec::new() },
            path: path.into_os_string(),
        }
    }

    fn get_children(src: &std::path::PathBuf) -> Vec<Self> {
        let mut result = Vec::new();

        let Ok(read_dir) = std::fs::read_dir(src) else {
            return result;
        };

        for dir_entry in read_dir {
            if let Ok(dir_entry) = dir_entry {
                result.push(Self::from_dir_entry(&dir_entry));
            }
        }

        result.sort_by(|l,r| l.name.as_str().cmp(r.name.as_str()));

        result
    }
}
