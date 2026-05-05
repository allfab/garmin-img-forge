use crate::typ::TypDocument;

pub struct App {
    pub doc: Option<TypDocument>,
    pub current_file_path: Option<std::path::PathBuf>,
    pub dirty: bool,
}

impl App {
    pub fn new() -> Self {
        Self { doc: None, current_file_path: None, dirty: false }
    }

    pub fn open_txt(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        let bytes = std::fs::read(path)?;
        self.doc = Some(crate::typ::text_reader::parse(&bytes)?);
        self.current_file_path = Some(path.to_path_buf());
        self.dirty = false;
        Ok(())
    }

    pub fn save_txt(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        if let Some(ref doc) = self.doc {
            let bytes = crate::typ::text_writer::write(doc)?;
            std::fs::write(path, bytes)?;
            self.current_file_path = Some(path.to_path_buf());
            self.dirty = false;
        }
        Ok(())
    }

    pub fn export_typ(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if let Some(ref doc) = self.doc {
            let bytes = crate::typ::binary_writer::compile(doc)?;
            std::fs::write(path, bytes)?;
        }
        Ok(())
    }

    pub fn import_typ(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        let bytes = std::fs::read(path)?;
        self.doc = Some(crate::typ::binary_reader::decompile(&bytes)?);
        // Pas de current_file_path : le source binaire n'est pas le fichier TXT éditable
        self.dirty = false;
        Ok(())
    }
}
