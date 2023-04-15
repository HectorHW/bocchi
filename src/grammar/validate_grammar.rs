use std::collections::HashSet;

use anyhow::anyhow;
use beau_collector::BeauCollector;

use super::{Grammar, Token};

type ValidateResult = Result<(), anyhow::Error>;

pub fn validate_grammar(g: &Grammar) -> ValidateResult {
    let checks = [find_root, resolve_names];

    let _ = checks
        .into_iter()
        .map(|check| check(g))
        .bcollect::<Vec<_>>()?;

    Ok(())
}

fn find_root(g: &Grammar) -> ValidateResult {
    if !g.productions.contains_key("root") {
        Err(anyhow!("provided grammar does not contain node `root`"))
    } else {
        Ok(())
    }
}

fn resolve_names(g: &Grammar) -> ValidateResult {
    let mut errors = HashSet::new();

    for productions in &g.productions {
        for production in productions.1 {
            for token in production {
                let Token::Identifier(i) = token else {
                    continue;
                };

                if !g.productions.contains_key(i) {
                    errors.insert(i.clone());
                }
            }
        }
    }

    errors
        .into_iter()
        .map(|e| {
            Err::<(), anyhow::Error>(anyhow!(
                "production `{e}` is mentioned in grammar but not defined"
            ))
        })
        .bcollect::<Vec<_>>()?;
    Ok(())
}
