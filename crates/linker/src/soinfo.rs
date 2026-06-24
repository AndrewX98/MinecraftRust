use std::collections::HashMap;

#[derive(Clone, Default, Debug)]
pub struct SoInfo {
    pub name: String,
    pub soname: String,
    pub base: usize,
    pub size: usize,
    pub dynamic: Option<usize>,
    pub symtab: Option<usize>,
    pub symtab_size: usize,
    pub strtab: Option<usize>,
    pub strtab_size: usize,
    pub gnu_hash: Option<usize>,
    pub sysv_hash: Option<usize>,
    pub bucket_count: usize,
    pub bucket: Vec<u32>,
    pub chain: Vec<u32>,
    pub gnu_bucket: Vec<u32>,
    pub gnu_chain: Vec<u32>,
    pub gnu_bloom_filter: Vec<usize>,
    pub gnu_bloom_shift: usize,
    pub gnu_bloom_n: usize,
    pub pltrel: Option<(usize, usize)>,
    pub pltrel_type: RelocType,
    pub rel: Option<(usize, usize)>,
    pub rela: Option<(usize, usize)>,
    pub rel_size: usize,
    pub init: Option<usize>,
    pub init_array: Option<(usize, usize)>,
    pub fini: Option<usize>,
    pub fini_array: Option<(usize, usize)>,
    pub preinit_array: Option<(usize, usize)>,
    pub tls_module: Option<usize>,
    pub dependencies: Vec<String>,
    pub external_symbols: HashMap<String, usize>,
    pub is_stub: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RelocType {
    Rel,
    Rela,
}

impl Default for RelocType {
    fn default() -> Self {
        RelocType::Rela
    }
}
