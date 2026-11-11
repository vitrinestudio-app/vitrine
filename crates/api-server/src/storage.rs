//! Files on disk: `<data>/templates/<id>/template.vitrine-template` (the
//! checked, re-packed package) and `previews/<canvas>.png` (rendered here).

use std::path::PathBuf;

use vitrine_core::template::Package;

const PACKAGE: &str = "template.vitrine-template";

#[derive(Debug)]
pub struct Storage {
    root: PathBuf,
}

impl Storage {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn dir(&self, id: &str) -> PathBuf {
        // Ids are checked template ids (no separators).
        self.root.join(id)
    }

    /// Writes the package and its previews (replacing an older version).
    pub fn put(&self, pkg: &Package) -> anyhow::Result<u64> {
        let id = &pkg.manifest.id;
        let staging = self.root.join(format!(".staging-{id}"));
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::create_dir_all(staging.join("previews"))?;
        let bytes = pkg.to_zip_bytes()?;
        std::fs::write(staging.join(PACKAGE), &bytes)?;
        for c in &pkg.manifest.canvases {
            if let Some(png) = pkg.preview(&c.id) {
                std::fs::write(staging.join("previews").join(format!("{}.png", c.id)), png)?;
            }
        }
        let dir = self.dir(id);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::rename(&staging, &dir)?;
        Ok(bytes.len() as u64)
    }

    pub fn package(&self, id: &str) -> std::io::Result<Vec<u8>> {
        std::fs::read(self.dir(id).join(PACKAGE))
    }

    pub fn preview(&self, id: &str, canvas: &str) -> std::io::Result<Vec<u8>> {
        if !vitrine_core::model::is_valid_id(canvas) {
            return Err(std::io::ErrorKind::NotFound.into());
        }
        std::fs::read(self.dir(id).join("previews").join(format!("{canvas}.png")))
    }
}
