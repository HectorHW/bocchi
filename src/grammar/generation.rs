use std::io::Write;

use rand::Rng;
use rand_regex::Regex;

use crate::grammar::{Grammar, Token};

#[derive(Clone, Debug)]
pub struct ProductionApplication {
    pub rule_name: String,
    pub production_variant: usize,
    pub items: Vec<TreeNode>,
}

#[derive(Clone, Debug)]
pub struct TreeNode {
    pub start: usize,
    pub size: usize,
    pub item: TreeNodeItem,
}

#[derive(Clone, Debug)]
pub enum TreeNodeItem {
    ProductionApplication(ProductionApplication),
    String(String),
    HexString(Vec<u8>),
    Regex(String),
}

impl TreeNodeItem {
    fn find_tree_span(&self) -> usize {
        match self {
            TreeNodeItem::ProductionApplication(p) => p.items.iter().map(|item| item.size).sum(),
            TreeNodeItem::String(s) => s.len(),
            TreeNodeItem::HexString(s) => s.len(),
            TreeNodeItem::Regex(s) => s.len(),
        }
    }

    fn with_offset(self, offset: usize) -> TreeNode {
        TreeNode {
            start: offset,
            size: self.find_tree_span(),
            item: self,
        }
    }
}

impl From<TreeNodeItem> for TreeNode {
    fn from(value: TreeNodeItem) -> Self {
        TreeNode {
            start: 0,
            size: value.find_tree_span(),
            item: value,
        }
    }
}

impl TreeNode {
    /// write this tree to buffer setting indices in the process
    pub fn fold(&mut self, buffer: &mut Vec<u8>) {
        let before = buffer.len();
        match &mut self.item {
            TreeNodeItem::ProductionApplication(pa) => {
                for item in &mut pa.items {
                    item.fold(buffer);
                }
            }
            TreeNodeItem::String(s) => {
                buffer.write_all(s.as_bytes()).unwrap();
            }
            TreeNodeItem::HexString(s) => {
                buffer.write_all(s).unwrap();
            }
            TreeNodeItem::Regex(re) => {
                buffer.write_all(re.as_bytes()).unwrap();
            }
        }
        self.start = before;
        self.size = buffer.len() - before;
    }

    pub fn fold_into_sample(mut self) -> GrammarSample {
        let mut buf = vec![];

        self.fold(&mut buf);

        GrammarSample {
            tree: self,
            folded: buf,
        }
    }
}

impl From<TreeNode> for GrammarSample {
    fn from(mut val: TreeNode) -> Self {
        let mut folded = vec![];
        val.fold(&mut folded);
        GrammarSample { tree: val, folded }
    }
}

#[derive(Clone, Debug)]
pub struct GrammarSample {
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

    pub fn generate(&self) -> GrammarSample {
        let mut tree = loop {
            if let Ok(res) = self.generate_production("root", self.depth_limit) {
                break res;
            }
        };

        let mut folded = vec![];
        tree.fold(&mut folded);

        GrammarSample { tree, folded }
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
            Token::String(s) => Ok(TreeNodeItem::String(s.clone()).into()),
            Token::Hex(h) => Ok(TreeNodeItem::HexString(h.clone()).into()),

            Token::Regex(re) => {
                let regex_application = self.generate_regex(re);
                Ok(TreeNodeItem::Regex(regex_application).into())
            }

            &Token::Bytes { min, max } => {
                Ok(TreeNodeItem::HexString(self.generate_byte_sequence(min, max)).into())
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
