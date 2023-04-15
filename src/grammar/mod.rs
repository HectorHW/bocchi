pub mod generation;
mod grammar;
mod validate_grammar;

use grammar::grammar_parser::grammar;
pub use grammar::Grammar;
pub use grammar::Token;

pub use generation::Sample;
pub use generation::TreeNode;

pub fn parse_grammar(content: &str) -> Result<Grammar, anyhow::Error> {
    let parsed = grammar(content)?;

    validate_grammar::validate_grammar(&parsed)?;
    Ok(parsed)
}
