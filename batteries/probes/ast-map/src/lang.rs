pub struct Table {
    pub ext: &'static [&'static str],
    pub language: fn() -> tree_sitter::Language,
    pub kinds: &'static [(&'static str, &'static str)],
    pub shape_fields: &'static [&'static str],
}

pub const RUST: Table = Table {
    ext: &["rs"],
    language: || tree_sitter_rust::LANGUAGE.into(),
    kinds: &[
        ("function_item", "function"),
        ("mod_item", "module"),
        ("function_signature_item", "function"),
        ("closure_expression", "function"),
        ("struct_item", "type"),
        ("enum_item", "type"),
        ("union_item", "type"),
        ("trait_item", "type"),
        ("type_item", "type"),
        ("field_declaration", "field"),
        ("use_declaration", "import"),
        ("call_expression", "call"),
        ("macro_invocation", "call"),
    ],
    shape_fields: &["parameters", "return_type", "type"],
};

pub const TABLES: &[&Table] = &[&RUST];

pub fn for_path(path: &str) -> Option<&'static Table> {
    let ext = path.rsplit_once('.').map(|(_, e)| e)?;
    TABLES.iter().copied().find(|t| t.ext.contains(&ext))
}

pub fn normalize(t: &Table, native: &str) -> Option<&'static str> {
    t.kinds.iter().find(|(n, _)| *n == native).map(|(_, k)| *k)
}
