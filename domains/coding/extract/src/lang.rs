pub enum Vis {
    Child(&'static str),
    Ancestor {
        kind: &'static str,
        label: &'static str,
    },
    LeadingUpper(&'static str),
    Absent,
}

pub enum Attrs {
    Before(&'static str),
    Absent,
}

pub const NOISE: &[&str] = &[
    "allow", "warn", "deny", "expect", "inline", "cold", "doc", "rustfmt", "clippy",
];

pub struct Table {
    pub ext: &'static [&'static str],
    pub language: fn() -> tree_sitter::Language,
    pub kinds: &'static [(&'static str, &'static str)],
    pub shape_fields: &'static [&'static str],
    pub shape_kinds: &'static [&'static str],
    pub names: &'static [&'static str],
    pub attrs: Attrs,
    pub vis: Vis,
    pub name_from_parent: &'static [&'static str],
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
    shape_fields: &["type_parameters", "parameters", "return_type", "type"],
    shape_kinds: &["function_modifiers", "where_clause"],
    names: &["name", "function", "argument"],
    attrs: Attrs::Before("attribute_item"),
    vis: Vis::Child("visibility_modifier"),
    name_from_parent: &[],
};

const TS_KINDS: &[(&str, &str)] = &[
    ("function_declaration", "function"),
    ("generator_function_declaration", "function"),
    ("function_expression", "function"),
    ("arrow_function", "function"),
    ("method_definition", "function"),
    ("method_signature", "function"),
    ("function_signature", "function"),
    ("class_declaration", "type"),
    ("abstract_class_declaration", "type"),
    ("interface_declaration", "type"),
    ("type_alias_declaration", "type"),
    ("enum_declaration", "type"),
    ("internal_module", "module"),
    ("public_field_definition", "field"),
    ("property_signature", "field"),
    ("import_statement", "import"),
    ("call_expression", "call"),
    ("new_expression", "call"),
];

const TS_SHAPE: &[&str] = &["type_parameters", "parameters", "return_type", "type"];

const TS_SHAPE_KINDS: &[&str] = &["async", "abstract", "readonly"];

const TS_NAMES: &[&str] = &["name", "function", "source", "constructor"];

const TS_NAME_FROM_PARENT: &[&str] = &["variable_declarator", "pair"];

const TS_VIS: Vis = Vis::Ancestor {
    kind: "export_statement",
    label: "export",
};

pub const TYPESCRIPT: Table = Table {
    ext: &["ts", "mts", "cts"],
    language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
    kinds: TS_KINDS,
    shape_fields: TS_SHAPE,
    shape_kinds: TS_SHAPE_KINDS,
    names: TS_NAMES,
    attrs: Attrs::Before("decorator"),
    vis: TS_VIS,
    name_from_parent: TS_NAME_FROM_PARENT,
};

pub const TSX: Table = Table {
    ext: &["tsx", "jsx", "js", "mjs", "cjs"],
    language: || tree_sitter_typescript::LANGUAGE_TSX.into(),
    kinds: TS_KINDS,
    shape_fields: TS_SHAPE,
    shape_kinds: TS_SHAPE_KINDS,
    names: TS_NAMES,
    attrs: Attrs::Before("decorator"),
    vis: TS_VIS,
    name_from_parent: TS_NAME_FROM_PARENT,
};

pub const PYTHON: Table = Table {
    ext: &["py", "pyi"],
    language: || tree_sitter_python::LANGUAGE.into(),
    kinds: &[
        ("function_definition", "function"),
        ("lambda", "function"),
        ("class_definition", "type"),
        ("import_statement", "import"),
        ("import_from_statement", "import"),
        ("call", "call"),
    ],
    shape_fields: &["type_parameters", "parameters", "return_type", "type"],
    shape_kinds: &["async"],
    names: &["name", "function", "module_name"],
    attrs: Attrs::Before("decorator"),
    vis: Vis::Absent,
    name_from_parent: &[],
};

pub const GO: Table = Table {
    ext: &["go"],
    language: || tree_sitter_go::LANGUAGE.into(),
    kinds: &[
        ("function_declaration", "function"),
        ("method_declaration", "function"),
        ("func_literal", "function"),
        ("type_declaration", "type"),
        ("type_spec", "type"),
        ("field_declaration", "field"),
        ("import_declaration", "import"),
        ("call_expression", "call"),
    ],
    shape_fields: &["type_parameters", "parameters", "result", "type"],
    shape_kinds: &[],
    names: &["name", "function", "path"],
    attrs: Attrs::Absent,
    vis: Vis::LeadingUpper("export"),
    name_from_parent: &[],
};

pub const TABLES: &[&Table] = &[&RUST, &TYPESCRIPT, &TSX, &PYTHON, &GO];

pub fn for_path(path: &str) -> Option<&'static Table> {
    let ext = path.rsplit_once('.').map(|(_, e)| e)?;
    TABLES.iter().copied().find(|t| t.ext.contains(&ext))
}

pub fn normalize(t: &Table, native: &str) -> Option<&'static str> {
    t.kinds.iter().find(|(n, _)| *n == native).map(|(_, k)| *k)
}
