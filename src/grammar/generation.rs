use rand::Rng;
use rand_regex::Regex;

use crate::{
    grammar::{Grammar, Token},
    sample::{GrammarSample, ProductionApplication, TreeNode, TreeNodeItem},
};

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

    pub fn generate(&self) -> GrammarSample {
        let tree = loop {
            if let Ok(res) = self.generate_production("root", self.depth_limit) {
                break res;
            }
        };

        tree.into()
    }

    pub fn generate_of_type(
        &self,
        name: &str,
        attempts: usize,
    ) -> Result<ProductionApplication, ()> {
        for _attempt in 0..attempts {
            if let Ok(TreeNode {
                item: TreeNodeItem::ProductionApplication(res),
                ..
            }) = self.generate_production(name, self.depth_limit)
            {
                return Ok(res);
            }
        }

        Err(())
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
            Token::String(s) => Ok(TreeNodeItem::Data(s.clone().into_bytes()).into()),
            Token::Hex(h) => Ok(TreeNodeItem::Data(h.clone()).into()),

            Token::Regex(re) => {
                let regex_application = self.generate_regex(re);
                Ok(TreeNodeItem::Data(regex_application.into_bytes()).into())
            }

            &Token::Bytes { min, max } => {
                Ok(TreeNodeItem::Data(self.generate_byte_sequence(min, max)).into())
            }
        }
    }

    fn generate_regex(&self, regex: &Regex) -> String {
        let mut rng = rand::thread_rng();
        rng.sample(regex)
    }

    fn generate_byte_sequence(&self, min: usize, max: usize) -> Vec<u8> {
        let mut rng = rand::thread_rng();

        let size = rng.gen_range(min..=max);

        (0..size).map(|_| rng.gen()).collect()
    }

    fn generate_production(
        &self,
        current_production: &str,
        remaining_depth: usize,
    ) -> Result<TreeNode, ()> {
        let productions = self.grammar.productions.get(current_production).unwrap_or_else(|| {
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
                return Ok(TreeNodeItem::ProductionApplication(ProductionApplication {
                    rule_name: current_production.to_string(),
                    production_variant: chosen_idx,
                    items: sub,
                })
                .into());
            }
        }

        Err(())
    }
}
