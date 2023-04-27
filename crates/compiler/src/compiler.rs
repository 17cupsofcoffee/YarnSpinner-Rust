//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/da39c7195107d8211f21c263e4084f773b84eaff/YarnSpinner.Compiler/Compiler.cs>

pub(crate) use self::{antlr_rust_ext::*, utils::*};
use crate::output::*;
use crate::prelude::FileParseResult;
use crate::string_table_manager::StringTableManager;
use crate::visitors::*;
use antlr_rust::tree::ParseTreeVisitorCompat;
pub use compilation_job::*;
use rusty_yarn_spinner_core::prelude::Library;
use rusty_yarn_spinner_core::types::*;
use std::collections::HashSet;

mod antlr_rust_ext;
mod compilation_job;
mod utils;

/// Compile Yarn code, as specified by a compilation job.
pub fn compile(compilation_job: CompilationJob) -> CompilationResult {
    // TODO: other steps
    let compiler_steps: Vec<&CompilationStep> = vec![
        &register_strings,
        &get_declarations,
        &find_tracking_nodes,
        &add_tracking_declarations,
    ];

    let initial = CompilationIntermediate::from_job(&compilation_job);
    compiler_steps
        .into_iter()
        .fold(initial, |state, step| step(state))
        .result
}

type CompilationStep = dyn Fn(CompilationIntermediate) -> CompilationIntermediate;

fn get_declarations(mut state: CompilationIntermediate) -> CompilationIntermediate {
    // Find the variable declarations in these files.
    for file in &state.parsed_files {
        let mut variable_declaration_visitor = DeclarationVisitor::new(
            file.name.clone(),
            state.job.variable_declarations.clone(),
            file.tokens(),
        );

        variable_declaration_visitor.visit(&*file.tree);
        let result = &mut state.result;
        result
            .diagnostics
            .extend_from_slice(&variable_declaration_visitor.diagnostics);
        result
            .declarations
            .extend(variable_declaration_visitor.new_declarations);
        result
            .file_tags
            .insert(file.name.clone(), variable_declaration_visitor.file_tags);
    }
    state
}

fn register_strings(mut state: CompilationIntermediate) -> CompilationIntermediate {
    // First pass: parse all files, generate their syntax trees,
    // and figure out what variables they've declared
    let mut string_table_manager: StringTableManager = state.result.string_table.into();
    for file in &state.job.files {
        let parse_result = parse_syntax_tree(file, &mut state.result.diagnostics);

        // ok now we will add in our lastline tags
        // we do this BEFORE we build our strings table otherwise the tags will get missed
        // this should probably be a flag instead of every time though
        let mut last_line_tagger = LastLineBeforeOptionsVisitor::default();
        last_line_tagger.visit(&*parse_result.tree);

        let mut visitor = StringTableGeneratorVisitor::new(
            file.file_name.clone(),
            string_table_manager.clone(),
            parse_result.tokens(),
        );
        visitor.visit(&*parse_result.tree);
        state.result.diagnostics.extend(visitor.diagnostics);
        string_table_manager.extend(visitor.string_table_manager);
        state.parsed_files.push(parse_result);
    }
    state.result.string_table = string_table_manager.into();
    state
}

fn find_tracking_nodes(mut state: CompilationIntermediate) -> CompilationIntermediate {
    // determining the nodes we need to track visits on
    // this needs to be done before we finish up with declarations
    // so that any tracking variables are included in the compiled declarations
    let mut tracking_nodes = HashSet::new();
    let mut ignore_nodes = HashSet::new();
    for file in &state.parsed_files {
        let mut visitor = NodeTrackingVisitor::new();
        visitor.visit(&*file.tree);
        tracking_nodes.extend(visitor.tracking_nodes);
        ignore_nodes.extend(visitor.ignoring_nodes);
    }
    state.tracking_nodes = tracking_nodes.difference(&ignore_nodes).cloned().collect();
    state
}

fn add_tracking_declarations(mut state: CompilationIntermediate) -> CompilationIntermediate {
    let tracking_declarations: Vec<_> = state
        .tracking_nodes
        .iter()
        .map(|node| {
            Declaration::from_default_value(0.)
                .with_name(Library::generate_unique_visited_variable_for_node(node))
                .with_type(NumberType)
                .with_description(format!(
                    "The generated variable for tracking visits of node {node}"
                ))
        })
        .collect();

    // adding the generated tracking variables into the declaration list
    // this way any future variable storage system will know about them
    // if we didn't do this later stages wouldn't be able to interface with them
    state
        .known_variable_declarations
        .extend(tracking_declarations.clone());
    state
        .derived_variable_declarations
        .extend(tracking_declarations);
    state
}

struct CompilationIntermediate<'input> {
    pub(crate) job: &'input CompilationJob,
    pub(crate) result: CompilationResult,
    pub(crate) known_variable_declarations: Vec<Declaration>,
    pub(crate) derived_variable_declarations: Vec<Declaration>,
    pub(crate) parsed_files: Vec<FileParseResult<'input>>,
    pub(crate) tracking_nodes: HashSet<String>,
}

impl<'input> CompilationIntermediate<'input> {
    pub(crate) fn from_job(compilation_job: &'input CompilationJob) -> Self {
        Self {
            job: compilation_job,
            result: Default::default(),
            known_variable_declarations: Default::default(),
            derived_variable_declarations: Default::default(),
            parsed_files: Default::default(),
            tracking_nodes: Default::default(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn can_call_compile_empty_without_crash() {
        compile(CompilationJob {
            files: vec![],
            library: None,
            compilation_type: CompilationType::FullCompilation,
            variable_declarations: vec![],
        });
    }

    #[test]
    fn can_call_compile_file_without_crash() {
        let file = File {
            file_name: "test.yarn".to_string(),
            source: "title: test
---
foo
bar
a {1 + 3} cool expression
==="
            .to_string(),
        };
        compile(CompilationJob {
            files: vec![file],
            library: None,
            compilation_type: CompilationType::FullCompilation,
            variable_declarations: vec![],
        });
    }
}