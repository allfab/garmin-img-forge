use crate::typ::TypDocument;

pub struct App {
    pub doc: Option<TypDocument>,
}

impl App {
    pub fn new() -> Self {
        Self { doc: None }
    }

    pub fn open_txt(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        let bytes = std::fs::read(path)?;
        self.doc = Some(crate::typ::text_reader::parse(&bytes)?);
        Ok(())
    }

    pub fn save_txt(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if let Some(ref doc) = self.doc {
            let bytes = crate::typ::text_writer::write(doc)?;
            std::fs::write(path, bytes)?;
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
        Ok(())
    }
}
