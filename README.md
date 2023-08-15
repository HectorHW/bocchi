# bocchifuzz

Hybrid graybox fuzzer that uses ptrace for coverage measurement.

**Note**:  This project is still WIP, some functionality (eg advanced mutations or additional grammar utilities) might be missing.

## Usage

Fuzzer can be used in two possible ways:

1. Classic seed-based fuzzing. When configured this way, fuzzer expects a set of examples from which new samples will be generated using binary mutations.

2. Grammar-based fuzzing. This mode allows to supply grammar that describes input structure thus enabling fuzzing programs that expect some form of structured input (eg. video decoders, programming language interpreters and so on).

After configuring the fuzzer (via `fuzz.toml`, see *Configuration*), run it either in project directory via `cargo run --release` or simply `bocchifuzz` (if using compiled binary).

## Configuration

Fuzzer is configured via `fuzz.toml` file using TOML syntax.

### Binary Configuration

Fuzzer expects a path to binary as well as optional input passing specification (file or stdin).

```toml
[binary]
path = "samples/exif/exif"
pass_style = "file"  # defaults to "stdin"
```

### Output configuration

During fuzzing new samples that cause program crash are saved under output directory. To modify it, use `[output]` section.

```toml
[output]
directory = "crashes"  # defaults to "output"
```

### Mode A - binary fuzzing

To use binary fuzzing, assign samples directory to `input.seeds` config key key.

```toml
[input]
seeds = "samples/exif/examples"
```

### Mode B - grammar fuzzing

To use grammar fuzzing, create appropriate input description and set path to grammar.

```toml
[input]
grammar = "path/to/my.grammar"
```

### Grammar syntax

Context-free grammar is used for input description, grammar is represented by a set of inference rules (in a form of `Nonterminal -> rhs`). Every grammar is expected to have at least one rule with nonterminal called `root` which is used  as base for input bulding. Each rule represents a set of alternatives separated by pipe `|`. Each alternative is a list of tokens. Tokens that can be placed on the right side of `->` are:

* Nonterminals allowing to create recursive rules (eg `root -> header body`)
* Strings in quotes allowing to describe text to be inserted (`"SELECT"`)
* Regular expressions (written as `re("pattern")`) allowing for simpler text entry definitions. Additionally two arguments can be passed: `size_limit` to limit number of expansions performed on `*` and `+` while generating text according to regex and `unicode` to enable generation of non-ascii characters. Example: `re("[a-z]+" size_limit=10)`
* Byte sequences encoded as hex (eg. `0x1D1F`). Such tokens can be useful for specifying magic bytes of a file
* Byte placeholders that allow fuzzer to insert random data with given size (`bytes(4)`). Size can be either a single number which is interpreted as exact number of bytes to insert or lower and upper bound (`bytes(4 8)`)
