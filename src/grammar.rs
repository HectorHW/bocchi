use rand_regex::Regex;

use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum Token {
    Identifier(String),
    String(String),
    Hex(Vec<u8>),
    Regex(Regex),
}

pub type ProductionRhs = Vec<Token>;

#[derive(Clone, Debug)]
pub struct Production {
    pub lhs: String,
    pub rhs: Vec<ProductionRhs>,
}

pub struct Grammar {
    pub options: HashMap<String, String>,

    pub productions: HashMap<String, Vec<ProductionRhs>>,
}

fn compile_regex(s: &str, size_limit: u32) -> Result<Regex, &'static str> {
    let mut parser = regex_syntax::ParserBuilder::new().unicode(false).build();
    let hir = parser.parse(s).map_err(|_| "error compiling regex")?;
    Ok(rand_regex::Regex::with_hir(hir, size_limit).unwrap())
}

peg::parser! {

    pub grammar grammar_parser() for str {

        rule flag() -> (String, String) =
            key:identifier() _ "=" _ value: flag_value() {
                (key, value)
            }

        rule flag_value() -> String =
            string()
            /
            n:number() {
                n.to_string()
            }

        rule flags() -> HashMap<String, String> =
            f:flag()**_ {
                f.into_iter().collect()
            }

        rule hexstring() -> Vec<u8> =
            "0x" hexblock:$(['0'..='9'|'a'..='f'|'A'..='F']+) {?
                if hexblock.as_bytes().len() % 2 != 0 {
                    return Err("number of hex digits should be even for hex block");
                }
                Ok(
                    (0..hexblock.len()).step_by(2).map(|i| u8::from_str_radix(&hexblock[i..i+2], 16).unwrap()).collect()
                )
            }

        rule stringchar() -> char =
            s:"\\\"" {'"'}
            /
            c:$([^'"']) {
                c.chars().next().unwrap()
            }


        rule string() -> String =
            "\"" s:stringchar()+ "\"" {
                s.iter().collect()
            }

        rule identifier() -> String =
            s:$(['a'..='z'|'A'..='Z'|'_']['a'..='z'|'A'..='Z'|'0'..='9'|'_']* ) {
                s.to_string()
            }

        rule number() -> u32 =
            s:$(['0'..='9']+) {
                s.parse().unwrap()
            }

        rule regex() -> Regex =
            "re" _ "(" _ s: string() _ f: flags() _ ")" {?

                let limit = f.get("size_limit")
                .map(|v| v.parse().map_err(|_| "error parsing number when reading regex size_limit flag")).unwrap_or(Ok(100))?;

                compile_regex(&s, limit)
            }


        rule token() -> Token =
            "Nothing" {
                Token::String("".to_string())
            }/
            r: regex() {
                Token::Regex(r)
            }/

            i:identifier() {
                Token::Identifier(i.to_string())
            }/
            s: string() {
                Token::String(s)
            }/
            hex: hexstring() {
                Token::Hex(hex)
            }

        rule rhs() -> ProductionRhs =
             token()++_

        rule more_rhs() -> ProductionRhs =
            _ "|" _ r:rhs() _ {r}


        rule production() -> Production =
            _ name: identifier() _ "->" _ first: rhs() _ rest: more_rhs()* _ ";" _ {
                let mut rest = rest;
                rest.insert(0, first);
                Production { lhs: name.to_string(), rhs: rest }
            }

        pub rule grammar() -> Grammar =
            _ f:flags() _
            prods: production()+ _ {
                Grammar{ options: f, productions: prods.into_iter().map(|p| (p.lhs, p.rhs)).collect() }

            }








        rule _() = quiet!{[' ' | '\r' | '\n' | '\t']*}

    }
}
