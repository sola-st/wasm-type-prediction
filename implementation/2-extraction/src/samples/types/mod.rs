//! Our own "type language", abstracted away from the DWARF format.
use std::fmt;

use gimli::DwAte;

pub mod parse;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Type(pub Vec<TypeToken>);

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some((last_token, tokens)) = self.0.split_last() {
            for token in tokens {
                token.fmt(f)?;
                f.write_str(" ")?;
            }
            last_token.fmt(f)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeToken {
    Unknown,

    Primitive(PrimitiveType),

    Pointer,
    Array,

    Const,

    Struct,
    Class,

    Union,
    Enum,

    Function,

    Nominal(Box<str>),
    Typedef(Box<str>),

    // Artificial token to make non-linear types unambiguous.
    End
}

impl fmt::Display for TypeToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use TypeToken::*;
        match self {
            Unknown => f.write_str("unknown"),
            Pointer => f.write_str("pointer"),
            Array => f.write_str("array"),
            // TODO Print primitive prefix, or just the inner name?
            Primitive(prim) => write!(f, "primitive {}", prim.normalized),
            // Primitive(prim) => write!(f, "{}", prim.normalized),
            Const => f.write_str("const"),
            Struct => f.write_str("struct"),
            Class => f.write_str("class"),
            Union => f.write_str("union"),
            Enum => f.write_str("enum"),
            Function => f.write_str("function"),
            // TODO Print nominal prefix?
            Nominal(name) => write!(f, "name {:?}", name),
            // Nominal(name) => write!(f, "{:?}", name),
            Typedef(name) => write!(f, "typedef {:?}", name),
            End => write!(f, "end"),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct PrimitiveType {
    // TODO should this be printing 'unsigned int 32' instead of 'uint32_t' (which is a single token and thus less 'inspectable' by the model?)
    pub normalized: &'static str,

    // Print (for the neural network) as normalized only, but keep source properties for debugging etc.
    pub source_name: Box<str>,
    pub encoding: DwAte,
    pub byte_size: u64,
}
