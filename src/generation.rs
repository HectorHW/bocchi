use std::io::Write;

use rand::Rng;
use rand_regex::Regex;

use crate::grammar::{Grammar, Token};

pub struct ProductionApplication {
    pub rule_name: String,
    pub production_variant: usize,
    pub items: Vec<TreeNode>,
}

pub enum TreeNode {
    ProductionApplication(ProductionApplication),
    String(String),
    HexString(Vec<u8>),
    Regex(String),
}

impl TreeNode {
    pub fn fold(&self, buffer: &mut Vec<u8>) {
        match self {
            TreeNode::ProductionApplication(pa) => {
                for item in &pa.items {
                    item.fold(buffer);
                }
            }
            TreeNode::String(s) => {
                buffer.write_all(s.as_bytes()).unwrap();
            }
            TreeNode::HexString(s) => {
                buffer.write_all(s).unwrap();
            }
            TreeNode::Regex(re) => {
                buffer.write_all(re.as_bytes()).unwrap();
            }
        }
    }
}

pub struct Sample {
    pub tree: TreeNode,
    pub folded: Vec<u8>,
}

pub struct Generator {
    grammar: Grammar,
    depth_limit: usize,
}

impl Generator {
    pub fn new(grammar: Grammar, depth_limit: usize) -> Generator {
        Generator {
            grammar,
            depth_limit,
        }
    }

    pub fn generate(&self) -> Sample {
        let tree = loop {
            if let Ok(res) = self.generate_production("root", self.depth_limit) {
                break res;
            }
        };

        let mut folded = vec![];
        tree.fold(&mut folded);

        Sample { tree, folded }
    }

    fn generate_token(&self, token: &Token, remaining_depth: usize) -> Result<TreeNode, ()> {
        match token {
            Token::Identifier(i) => {
                if remaining_depth == 0 {
                    Err(())
                } else {
                    self.generate_production(i, remaining_depth - 1)
                }
            }
            Token::String(s) => Ok(TreeNode::String(s.clone())),
            Token::Hex(h) => Ok(TreeNode::HexString(h.clone())),

            Token::Regex(re) => Ok(TreeNode::Regex(self.generate_regex(re))),
        }
    }

    fn generate_regex(&self, regex: &Regex) -> String {
        let mut rng = rand::thread_rng();
        rng.sample(regex)
    }

    fn generate_production(
        &self,
        current_production: &str,
        remaining_depth: usize,
    ) -> Result<TreeNode, ()> {
        let productions = self.grammar.get(current_production).unwrap_or_else(|| {
            panic!("could not find production rule with name `{current_production}` in supplied grammar during generation")
        });

        for _ in 0..remaining_depth {
            let chosen_idx = rand::thread_rng().gen_range(0..productions.len());
            let production = &productions[chosen_idx];

            if let Ok(sub) = production
                .iter()
                .map(|token| self.generate_token(token, remaining_depth - 1))
                .collect::<Result<Vec<TreeNode>, ()>>()
            {
                return Ok(TreeNode::ProductionApplication(ProductionApplication {
                    rule_name: current_production.to_string(),
                    production_variant: chosen_idx,
                    items: sub,
                }));
            }
        }

        Err(())
    }
}
