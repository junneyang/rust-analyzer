extern crate libsyntax2;
extern crate superslice;
extern crate itertools;

mod extend_selection;
mod symbols;
mod line_index;
mod edit;
mod code_actions;
mod typing;
mod completion;
mod scope;

use libsyntax2::{
    File, TextUnit, TextRange, SyntaxNodeRef,
    ast::{AstNode, NameOwner},
    algo::{walk, find_leaf_at_offset, ancestors},
    SyntaxKind::{self, *},
};
pub use libsyntax2::AtomEdit;
pub use self::{
    line_index::{LineIndex, LineCol},
    extend_selection::extend_selection,
    symbols::{StructureNode, file_structure, FileSymbol, file_symbols},
    edit::{EditBuilder, Edit},
    code_actions::{
        ActionResult,
        flip_comma, add_derive, add_impl,
    },
    typing::{join_lines, on_eq_typed},
    completion::scope_completion,
};

#[derive(Debug)]
pub struct HighlightedRange {
    pub range: TextRange,
    pub tag: &'static str,
}

#[derive(Debug)]
pub struct Diagnostic {
    pub range: TextRange,
    pub msg: String,
}

#[derive(Debug)]
pub struct Runnable {
    pub range: TextRange,
    pub kind: RunnableKind,
}

#[derive(Debug)]
pub enum RunnableKind {
    Test { name: String },
    Bin,
}

pub fn matching_brace(file: &File, offset: TextUnit) -> Option<TextUnit> {
    const BRACES: &[SyntaxKind] = &[
        L_CURLY, R_CURLY,
        L_BRACK, R_BRACK,
        L_PAREN, R_PAREN,
        L_ANGLE, R_ANGLE,
    ];
    let (brace_node, brace_idx) = find_leaf_at_offset(file.syntax(), offset)
        .filter_map(|node| {
            let idx = BRACES.iter().position(|&brace| brace == node.kind())?;
            Some((node, idx))
        })
        .next()?;
    let parent = brace_node.parent()?;
    let matching_kind = BRACES[brace_idx ^ 1];
    let matching_node = parent.children()
        .find(|node| node.kind() == matching_kind)?;
    Some(matching_node.range().start())
}

pub fn highlight(file: &File) -> Vec<HighlightedRange> {
    let mut res = Vec::new();
    for node in walk::preorder(file.syntax()) {
        let tag = match node.kind() {
            ERROR => "error",
            COMMENT | DOC_COMMENT => "comment",
            STRING | RAW_STRING | RAW_BYTE_STRING | BYTE_STRING => "string",
            ATTR => "attribute",
            NAME_REF => "text",
            NAME => "function",
            INT_NUMBER | FLOAT_NUMBER | CHAR | BYTE => "literal",
            LIFETIME => "parameter",
            k if k.is_keyword() => "keyword",
            _ => continue,
        };
        res.push(HighlightedRange {
            range: node.range(),
            tag,
        })
    }
    res
}

pub fn diagnostics(file: &File) -> Vec<Diagnostic> {
    let mut res = Vec::new();

    for node in walk::preorder(file.syntax()) {
        if node.kind() == ERROR {
            res.push(Diagnostic {
                range: node.range(),
                msg: "Syntax Error".to_string(),
            });
        }
    }
    res.extend(file.errors().into_iter().map(|err| Diagnostic {
        range: TextRange::offset_len(err.offset, 1.into()),
        msg: err.msg,
    }));
    res
}

pub fn syntax_tree(file: &File) -> String {
    ::libsyntax2::utils::dump_tree(file.syntax())
}

pub fn runnables(file: &File) -> Vec<Runnable> {
    file.ast()
        .functions()
        .filter_map(|f| {
            let name = f.name()?.text();
            let kind = if name == "main" {
                RunnableKind::Bin
            } else if f.has_atom_attr("test") {
                RunnableKind::Test {
                    name: name.to_string()
                }
            } else {
                return None;
            };
            Some(Runnable {
                range: f.syntax().range(),
                kind,
            })
        })
        .collect()
}

pub fn find_node_at_offset<'a, N: AstNode<'a>>(
    syntax: SyntaxNodeRef<'a>,
    offset: TextUnit,
) -> Option<N> {
    let leaves = find_leaf_at_offset(syntax, offset);
    let leaf = leaves.clone()
        .find(|leaf| !leaf.kind().is_trivia())
        .or_else(|| leaves.right_biased())?;
    ancestors(leaf)
        .filter_map(N::cast)
        .next()
}
