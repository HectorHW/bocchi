use anyhow::Context;

pub fn read_seeds(path: &str) -> Result<Vec<Vec<u8>>, anyhow::Error> {
    if std::fs::metadata(path)
        .context("trying to get info about seed path")?
        .is_dir()
    {
        let mut files = vec![];
        for entry in std::fs::read_dir(path).context("trying to enumerate seed directory")? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                eprintln!("encountered subdirectory {} in seed directory. Note that seed reading is non-recursive", 
                entry.file_name().to_string_lossy());
            } else {
                let path = entry.path();
                let content = std::fs::read(&path).context(format!(
                    "trying to read seed file {}",
                    path.to_string_lossy()
                ))?;
                files.push(content);
            }
        }

        if files.is_empty() {
            anyhow::bail!("read no files after reading specified seed directory");
        } else {
            Ok(files)
        }
    } else {
        let content = std::fs::read(path).context("trying to read seed file")?;
        Ok(vec![content])
    }
}
