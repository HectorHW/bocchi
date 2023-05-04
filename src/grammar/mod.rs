pub mod generation;
mod parse;
mod validate_grammar;

use parse::grammar_parser::grammar;
pub use parse::Grammar;
pub use parse::Token;

pub use generation::GrammarSample;
pub use generation::TreeNode;

pub fn parse_grammar(content: &str) -> Result<Grammar, anyhow::Error> {
    let parsed = grammar(content)?;

    validate_grammar::validate_grammar(&parsed)?;
    Ok(parsed)
}
