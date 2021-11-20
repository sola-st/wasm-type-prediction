use std::fmt;
use std::rc::Rc;

use gimli::{AttributeValue, DebuggingInformationEntry, DwAt, DwTag, Dwarf, Reader, ReaderOffset, Unit, UnitOffset};

// My own convenience wrapper around gimli::DebuggingInformationEntry, which has two problems: 
// 1. It has complicated lifetimes because it borrows from Unit and Abbreviations.
// 2. Most interesting operations on an entry require Unit/Dwarf anyway, so keep them together.
#[derive(Clone)]
pub struct DwarfEntry<R: Reader> {
    // Make the unit and global parser state an Rc/shared pointer as to not copy it around (they 
    // contain quite many fields) and and also keep down this struct's size.
    dwarf: Rc<Dwarf<R>>,
    unit: Rc<Unit<R>>,
    
    // Store only the offset instead of the DebuggingInformationEntry itself, because the latter
    // borrows from the Unit, which would make this a self-referential struct -.-
    // From what I understand, the only downside is that the entry is parsed multiple times, once 
    // before calling new, and once again everytime entry() is called. 
    // (However, from gimli::DebuggingInformationEntry::parse, that seems to be not very expensive.)
    entry_offset: UnitOffset<R::Offset>,

    // Store the tag in here, so that inspecting it doesn't cause a re-parse of the DIE.
    pub tag: DwTag,
}

// Do not write dwarf and unit fields with all their (gimli-specific) data to debug output, since
// that easily overwhelms the console (and is also quite useless for my debugging).
// One easy way to look at the original DWARF entry given this debug output is:
// llvm-dwarfdump <file> --debug-info=<offset> -c
impl<R: Reader> fmt::Debug for DwarfEntry<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DwarfEntry")
            .field("tag", &format!("{}", &self.tag))
            .field("offset", &format!("0x{:x}", self.entry_offset.0.into_u64()))
            .finish()
    }
}

impl<R: Reader> DwarfEntry<R> {
    pub fn from(
        dwarf: &Rc<Dwarf<R>>,
        unit: &Rc<Unit<R>>,
        entry: &DebuggingInformationEntry<R>,
    ) -> Self {
        Self { 
            dwarf: Rc::clone(dwarf),
            unit: Rc::clone(unit),
            entry_offset: entry.offset(),
            tag: entry.tag(),
        }
    }

    pub fn entry(&self) -> DebuggingInformationEntry<R> {
        self.unit.entry(self.entry_offset)
            .expect("since we constructed it from a valid entry, the unit should contain an entry at the offset")
    }

    /// Read a generic attribute with `name`.
    pub fn attr(&self, name: DwAt) -> gimli::Result<Option<AttributeValue<R>>> {
        self.entry().attr_value(name)
    }

    /// Read an attribute with `name` that resolves to another debugging information entry.
    pub fn attr_entry(&self, name: DwAt) -> gimli::Result<Option<Self>> {
        match self.attr(name)? {
            Some(AttributeValue::UnitRef(offset)) => {
                let entry = self.unit.entry(offset)?;
                let entry = Self::from(&self.dwarf, &self.unit, &entry);
                Ok(Some(entry))
            }
            Some(attr_value) => unimplemented!("unknown DWARF attribute value: {:?}", attr_value),
            None => Ok(None)
        }
    }

    /// Read a string attribute with `name`.
    pub fn attr_str(&self, name: DwAt) -> gimli::Result<Option<Box<str>>> {
        match self.attr(name)? {
            Some(attr_value) => {
                let data = self.dwarf.attr_string(&self.unit, attr_value)?;
                let str = data.to_string()?;
                // TODO I really would have like to avoid the allocation (Box<str>), but gimli's API
                // is really cumbersome for two reasons:
                // 1. R::to_string() gives you a Cow<str> instead of a plain &str, even though all
                //    their implementations (EndianRcSlice, EndianArcSlice) could give you a slice.
                // 2. The lifetime on the R::to_string() method in the trait definition is wrong, 
                //    because it gives the result the same lifetime as R, whereas it should be the 
                //    lifetime of the underlying input data.
                //    Thus, even if you WOULD use Cow<str> here, you get an invalid lifetime because
                //    it seems to borrow from the local variable `data` (which it doesn't -.-).
                //    (For EndianSlice<'a> the lifetime of to_string() is correct...)
                Ok(Some(str.into()))
            }
            None => Ok(None)
        }
    }
    
    /// Read an unsigned integer attribute with `name`.
    pub fn attr_uint(&self, name: DwAt) -> gimli::Result<Option<u64>> {
        let attr_value = match self.attr(name)? {
            Some(attr_value) => attr_value,
            None => return Ok(None)
        };

        Ok(Some(match attr_value {
            AttributeValue::Addr(u) => u as u64,
            AttributeValue::Data1(u) => u as u64,
            AttributeValue::Data2(u) => u as u64,
            AttributeValue::Data4(u) => u as u64,
            AttributeValue::Data8(u) => u as u64,
            AttributeValue::Udata(u) => u as u64,
            _ => unimplemented!("unknown DWARF attribute value: {:?}", attr_value),
        }))
    }
    
    /// Iterator over the direct children of this entry.
    pub fn children(&self) -> gimli::Result<ChildIter<R>> {
        let mut cursor = self.unit.entries_at_offset(self.entry_offset)?;
        // We need one invocation of next_dfs() to actually move the cursor to the given offset.
        cursor.next_dfs()?;
        // Do one further step DFS to go to the first child.
        cursor.next_dfs()?;
        Ok(ChildIter { 
            dwarf: Rc::clone(&self.dwarf),
            unit: Rc::clone(&self.unit), 
            cursor
        })
    }
    
}

pub struct ChildIter<'abbrev, 'unit, R: Reader> {
    dwarf: Rc<Dwarf<R>>,
    unit: Rc<Unit<R>>,
    cursor: gimli::read::EntriesCursor<'abbrev, 'unit, R>
}

impl<'abbrev, 'unit, R: Reader> Iterator for ChildIter<'abbrev, 'unit, R> {
    type Item = gimli::Result<DwarfEntry<R>>;

    fn next(&mut self) -> Option<Self::Item> {
        // This is a bit weird: Since the cursor from gimli caches the "current" (really the last)
        // parsed entry, we already have it available in the beginning, and THEN try to advance the
        // iterator with cursor.next_sibling(). If the next_sibling call fails, give back the error,
        // if it could advance, give back the current (converted) entry.
        let entry = self.cursor.current()
            .map(|entry| 
                DwarfEntry::from(&self.dwarf, &self.unit, entry));
        self.cursor.next_sibling().map(|_next| entry).transpose()
    }
}