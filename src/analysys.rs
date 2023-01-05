use std::path::Path;

#[derive(thiserror::Error, Debug)]
pub enum AnalysysError {
    #[error("failed to open binary for analysis: {0:?}")]
    IO(#[from] std::io::Error),

    #[error(transparent)]
    Goblin(#[from] goblin::error::Error),

    #[error("format error: {0}")]
    FileFormat(String),
}

pub struct Binary {
    pub functions: Vec<Function>,
}

pub struct Function {
    name: String,
    offset: usize,
}

pub fn analyze_binary<P: AsRef<Path>>(path: P) -> Result<Binary, AnalysysError> {
    let binary_data = std::fs::read(path)?;

    let elf = match goblin::Object::parse(&binary_data)? {
        goblin::Object::Elf(elf) => elf,

        goblin::Object::Unknown(magic) => {
            return Err(AnalysysError::FileFormat(format!(
                "Unknown file magic {magic}"
            )))
        }

        _ => {
            return Err(AnalysysError::FileFormat(
                "Unsupported binary type. Only elf is supported at the moment".to_string(),
            ))
        }
    };

    let functions = elf
        .syms
        .iter()
        .filter_map(|symbol| {
            if !symbol.is_function() {
                return None;
            }

            if symbol.st_value == 0 || symbol.st_size == 0 {
                return None;
            }

            let name = elf.strtab.get_at(symbol.st_name)?.to_string();
            let offset = symbol.st_value as usize;

            Some(Function { name, offset })
        })
        .collect();

    Ok(Binary { functions })
}
