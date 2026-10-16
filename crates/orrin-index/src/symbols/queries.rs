pub const RUST: &str = r#"
(function_item name: (identifier) @name) @function
(function_signature_item name: (identifier) @name) @function
(struct_item name: (type_identifier) @name) @struct
(union_item name: (type_identifier) @name) @struct
(enum_item name: (type_identifier) @name) @enum
(trait_item name: (type_identifier) @name) @trait
(impl_item type: (_) @name) @impl
(mod_item name: (identifier) @name) @module
(macro_definition name: (identifier) @name) @macro
(const_item name: (identifier) @name) @constant
(static_item name: (identifier) @name) @constant
(type_item name: (type_identifier) @name) @type
"#;

pub const SCRIPT: &str = r#"
(function_declaration name: (identifier) @name) @function
(generator_function_declaration name: (identifier) @name) @function
(method_definition name: (_) @name) @method
(variable_declarator name: (identifier) @name value: [(arrow_function) (function_expression)]) @function
"#;

pub const JAVASCRIPT: &str = r#"
(class_declaration name: (identifier) @name) @class
"#;

pub const TYPESCRIPT: &str = r#"
(class_declaration name: (type_identifier) @name) @class
(abstract_class_declaration name: (type_identifier) @name) @class
(interface_declaration name: (type_identifier) @name) @interface
(type_alias_declaration name: (type_identifier) @name) @type
(enum_declaration name: (identifier) @name) @enum
(internal_module name: (_) @name) @module
(method_signature name: (_) @name) @method
(abstract_method_signature name: (_) @name) @method
(function_signature name: (identifier) @name) @function
"#;

pub const PYTHON: &str = r#"
(class_definition name: (identifier) @name) @class
(function_definition name: (identifier) @name) @function
(module (expression_statement (assignment left: (identifier) @name) @variable))
"#;

pub const C: &str = r#"
(function_declarator declarator: (identifier) @name) @function
(struct_specifier name: (type_identifier) @name body: (_)) @struct
(union_specifier name: (type_identifier) @name body: (_)) @struct
(enum_specifier name: (type_identifier) @name body: (_)) @enum
(type_definition declarator: (type_identifier) @name) @type
(preproc_def name: (identifier) @name) @macro
(preproc_function_def name: (identifier) @name) @macro
"#;

pub const CPP: &str = r#"
(function_declarator declarator: (field_identifier) @name) @function
(function_declarator declarator: (qualified_identifier name: (identifier) @name)) @method
(class_specifier name: (type_identifier) @name body: (_)) @class
(namespace_definition name: (_) @name) @module
"#;

pub const GDSCRIPT: &str = r#"
(class_name_statement name: (name) @name) @class
(class_definition name: (name) @name) @class
(function_definition name: (name) @name) @function
(signal_statement name: (name) @name) @signal
(const_statement name: (name) @name) @constant
(enum_definition name: (name) @name) @enum
(variable_statement name: (name) @name) @variable
"#;
