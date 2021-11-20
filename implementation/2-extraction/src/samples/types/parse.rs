//! Parse the DWARF type format to our own, which abstracts over some language-specifcs and 
//! simplifies the type language a lot.
use anyhow::Context;
use gimli::{AttributeValue, Reader, constants::*};

use crate::dwarf::util::DwarfEntry;
use crate::samples::types::{Type, TypeToken, PrimitiveType};
use crate::samples::types::TypeToken::*;

impl Type {
    pub fn parse_param<R: Reader>(param_entry: &DwarfEntry<R>) -> anyhow::Result<Self> {
        if let Some(type_entry) = param_entry.attr_entry(DW_AT_type)? {
            Self::parse_type(&type_entry)
        } else {
            // For some parameters (about 1.9% of all samples), the DW_AT_type attribute is absent.
            // This seems to be the case for generic or inlined functions (not exactly sure) 
            // for which there then is instead a DW_AT_abstract_origin entry.
            // Then all information of children of that function (in particular, the types and names
            // of the parameters) is only available behind the abstract origin of the parameters 
            // also. So we try to resolve that here and otherwise say unknown type.
            if let Some(abstract_origin) = param_entry.attr_entry(DW_AT_abstract_origin)? {
                Self::parse_param(&abstract_origin)
            } else {
                Ok(Type(vec!(Unknown)))
            }
        }
    }

    pub fn parse_type<R: Reader>(type_entry: &DwarfEntry<R>) -> anyhow::Result<Self> {
        // Pre-allocate such that most types never need to grow (average: ~2.5 tokens per type).
        let mut tokens = Vec::with_capacity(4);

        Self::parse_type_to_tokens(&mut tokens, type_entry)?;

        Ok(Type(tokens))
    }

    fn parse_type_to_tokens<R: Reader>(tokens: &mut Vec<TypeToken>, entry: &DwarfEntry<R>) -> anyhow::Result<()> {
        #[allow(non_upper_case_globals)]
        match entry.tag {

            DW_TAG_base_type => {
                let prim = Self::parse_primitive_type(entry)?;
                tokens.push(Primitive(prim));
            },

            // Map C++ references (including C++11 rvalue references) to C pointers.
            DW_TAG_reference_type
            | DW_TAG_rvalue_reference_type
            // Map C++ "pointer to member" (apparently that has its own DWARF type!?) to plain pointers.
            | DW_TAG_ptr_to_member_type
            | DW_TAG_pointer_type => {
                tokens.push(Pointer);
                Self::parse_inner_type_to_tokens(tokens, entry)?;
            }

            DW_TAG_const_type => {
                tokens.push(Const);
                Self::parse_inner_type_to_tokens(tokens, entry)?;
            }

            DW_TAG_array_type => {
                tokens.push(Array);
                Self::parse_inner_type_to_tokens(tokens, entry)?;
            }

            DW_TAG_typedef => {
                // FIXME undecided whether typedefs should be included or flattened away.
                // There are two decisions to take here:
                // 1. (This one also applies to "named" classes, structs etc.): which names can be 
                // expected to be predictable, because they are common across different projects?
                // E.g., size_t - yes, but some_project_specific_name - no.
                // 2. Fundamentally, SHOULD typedefs even be distinct types from their definition?
                // In C, they are not (i.e., a typedef is interchangable with its underlying type)
                // but are still often used to signify semantics (and arguably often SHOULD be true
                // nominal types).
                let name = entry.attr_str(DW_AT_name)?.context("typedef must have DW_AT_name attribute")?;
                tokens.push(Typedef(name));
                Self::parse_inner_type_to_tokens(tokens, entry)?;
            },

            DW_TAG_enumeration_type => {
                if let Some(name) = entry.attr_str(DW_AT_name)? {
                    tokens.push(Nominal(name));
                }
                tokens.push(Enum);
                // The inner type of an enum is its primitive base type, I believe.
                Self::parse_inner_type_to_tokens(tokens, entry)?;
            }

            // TODO keep class vs. struct spearate? -> ablation study how well the model can handle this
            // // Map C++ classes to C structs (forgetting about all contained methods, virtuality, etc.)
            DW_TAG_class_type => {
                if let Some(name) = entry.attr_str(DW_AT_name)? {
                    tokens.push(Nominal(name));
                }
                tokens.push(Class);

                // FIXME currently not recursing into "types with multiple children"
            } 
            DW_TAG_structure_type => {
                if let Some(name) = entry.attr_str(DW_AT_name)? {
                    tokens.push(Nominal(name));
                }
                tokens.push(Struct);

                // FIXME currently not recursing into "types with multiple children"
            }

            DW_TAG_union_type => {
                if let Some(name) = entry.attr_str(DW_AT_name)? {
                    tokens.push(Nominal(name));
                }
                tokens.push(Union);

                // FIXME currently not recursing into "types with multiple children"
            }

            DW_TAG_subroutine_type => {
                tokens.push(Function)

                // FIXME currently not printing type of arguments and returns.
            },

            // Strip some type modifiers by just returning the inner type without wrapping
            DW_TAG_volatile_type
            | DW_TAG_restrict_type => Self::parse_inner_type_to_tokens(tokens, entry)?,

            // In general, the unspecified type can be a lot of things, the DWARF 5 standard, section 
            // 5.2, "Unspecified Type Entries" says:
            // "An unspecified (implicit, unknown, ambiguous or nonexistent) type"
            // "intentionally left flexible to allow it to be interpreted appropriately in different languages"
            // "in C and C++ the language implementation can provide an unspecified type entry with the 
            // name “void” which can be referenced by the type attribute of pointer types and typedef 
            // declarations for ’void’"
            // "C++ permits using the auto return type specifier for the return type of a member
            // function declaration. The actual return type is deduced based on the definition of the
            // function, so it may not be known when the function is declared. The language
            // implementation can provide an unspecified type entry with the name auto which can be
            // referenced by the return type attribute of a function declaration entry."
            //
            // However, in our dataset, all names of the unspecified type were exclusively
            // "decltype(nullptr)", which is the definition of the typedef "std::nullptr_t", which is
            // sometimes (but seldom) used in function definitions, see:
            // https://stackoverflow.com/questions/17069315/what-is-the-type-of-nullptr and
            // https://stackoverflow.com/questions/12066721/what-are-the-uses-of-the-type-stdnullptr-t
            //
            // We thus map an unspecified type with name "decltype(nulltpr)" to a pointer of unknown
            // type, and fail if there is an unexpected (or no) name for an unspecified type.
            DW_TAG_unspecified_type => {
                let name = entry.attr_str(DW_AT_name)?;
                match name.as_deref() {
                    Some("decltype(nullptr)") => {
                        tokens.push(Pointer);
                        tokens.push(Unknown);
                    },
                    name => anyhow::bail!("DW_TAG_unspecified_type with unexpected name: {:?}", name),
                };
            },
            tag => anyhow::bail!("unknown DW_AT_type entry tag: {}", tag)
        };

        Ok(())
    }

    fn parse_inner_type_to_tokens<R: Reader>(tokens: &mut Vec<TypeToken>, entry_with_type_attr: &DwarfEntry<R>) -> anyhow::Result<()> {
        if let Some(type_entry) = entry_with_type_attr.attr_entry(DW_AT_type)? {
            Self::parse_type_to_tokens(tokens, &type_entry)
        } else {
            tokens.push(Unknown);
            Ok(())
        }
    }

    fn parse_primitive_type<R: Reader>(entry: &DwarfEntry<R>) -> anyhow::Result<PrimitiveType> {
        let source_name = entry.attr_str(DW_AT_name)?
            .context("base (=primitive) type must have DW_AT_name attribute")?;
        
        let encoding = entry.attr(DW_AT_encoding)?
            .context("base (=primitive) type must have DW_AT_encoding attribute")?;
        let encoding = match encoding {
            AttributeValue::Encoding(encoding) => encoding,
            _ => unimplemented!("unknown DWARF attribute value: {:?}", encoding),
        };

        let byte_size = entry.attr_uint(DW_AT_byte_size)?
            .context("base (=primitive) type must have DW_AT_byte_size attribute")?;
            
        // Instead of just using the primitive type name as it appeared in the source code (and hence
        // in the DWARF name attribute), we are normalizing to "C-like" fixed-width type names. Reasons:
        // 1. to remove ambiguity from type names like "long" (typically 4 or 8 bytes, but could be even
        // larger), i.e., one type name in the source can correspond to different actual machine types.
        // 2. to merge together different type names for the same machine type, e.g., "short",
        // "short int", "signed short", and "signed short int" are all the same type, or "_Bool" and "bool".
        // 2. have each primitive type be a single token, otherwise it might become confusing for the
        // network that some primitive types are "long int" others just "long" (which are the same) and
        // yet others are "long double" (which is something completely different).
        #[allow(non_upper_case_globals)]
        let normalized = match (&source_name[..], encoding, byte_size) {
            // This is really weird/stupid in C: there are three DISTINCT types of char in the specification,
            //  - "plain" char
            //  - unsigned char
            //  - signed char
            // but only two representations (unsigned and signed). Char is supposed to be used for
            // character data only (i.e., strings), where signed and unsigned doesn't matter and
            // guarantees only 7 bits of storage, but that's enough/doesn't matter for ASCII anyway.
            // Since this is semantically different from explicit unsigned and signed, we should single that out.
            // See also https://en.cppreference.com/w/cpp/language/types:
            // "The signedness of char depends on the compiler and the target platform: the defaults for
            // ARM and PowerPC are typically unsigned, the defaults for x86 and x64 are typically signed."
            // and (my observation) on the build config, since some programs explicitly configure the
            // compiler to use unsigned char for char.
            ("char", DW_ATE_signed_char, 1) | ("char", DW_ATE_unsigned_char, 1) => "char",

            // Character types that allows to store at least UTF16/32 code points, standardized since C++11.
            // https://en.cppreference.com/w/cpp/language/types.
            // https://docs.microsoft.com/en-US/cpp/cpp/char-wchar-t-char16-t-char32-t?view=msvc-160
            ("char16_t", DW_ATE_UTF, 2) => "char16_t",
            ("char32_t", DW_ATE_UTF, 4) => "char32_t",

            // Treat chars with explicit signed/unsigned annotation as integers of 1 byte.
            (_, DW_ATE_signed_char, 1) => "int8_t",
            (_, DW_ATE_unsigned_char, 1) => "uint8_t",

            (_, DW_ATE_signed, 2) => "int16_t",
            (_, DW_ATE_signed, 4) => "int32_t",
            (_, DW_ATE_signed, 8) => "int64_t",
            (_, DW_ATE_unsigned, 2) => "uint16_t",
            (_, DW_ATE_unsigned, 4) => "uint32_t",
            (_, DW_ATE_unsigned, 8) => "uint64_t",

            // Exact-width floating point types do not exist in the C/C++ standards, but we believe the
            // following names are self-explanatory enough, and there is prior work for fixed-size
            // float typedefs in the Boost libraries:
            // https://www.boost.org/doc/libs/master/libs/math/doc/html/math_toolkit/exact_typdefs.html
            // Typical float types are "float" and "double" (32/64-bit, standard IEEE754), "long double"
            // (can be 80-bit extended precision floats of the x86 x87 FPU, but could also be 128-bit
            // IEEE 754, then also typedef'd to "__float128")
            (_, DW_ATE_float, 4) => "float32_t",
            (_, DW_ATE_float, 8) => "float64_t",
            (_, DW_ATE_float, 16) => "float128_t",

            // C99 has a keyword for complex numbers (guaranteed to have the same layout as a 2-element
            // float array), in C++ it is a standard library class, so not in the language itself.
            // https://en.cppreference.com/w/c/language/arithmetic_types#Complex_floating_types
            // https://gcc.gnu.org/onlinedocs/gcc/Complex.html
            (_, DW_ATE_complex_float, 16) => "complex",
            // There was only a single sample for a complex float (i.e., 2 4-byte floats), so we map
            // that to the same representation for simplicity.
            (_, DW_ATE_complex_float, 8) => "complex",

            // C99 has _Bool as a keyword (macro'd to bool), C++ bool directly:
            // https://en.cppreference.com/w/c/language/arithmetic_types#Boolean_type
            // https://en.cppreference.com/w/cpp/language/types
            (_, DW_ATE_boolean, 1) => "bool",

            _ => anyhow::bail!(
                "unknown primitive type: source_name={}, encoding={}, byte_size={}",
                source_name,
                encoding,
                byte_size
            ),
        };

        Ok(PrimitiveType { normalized, source_name, encoding, byte_size })
    }
}