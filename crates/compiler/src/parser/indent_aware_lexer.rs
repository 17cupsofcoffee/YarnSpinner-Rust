//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/da39c7195107d8211f21c263e4084f773b84eaff/YarnSpinner.Compiler/IndentAwareLexer.cs>
//!
//! The C# implementation uses inheritance to do this.
//! More specifically, the lexer generated by ANTLR derives from the `IndentAwareLexer`
//! directly, and the `IndentAwareLexer` derives from the ANTLR Lexer base class.
//! Instead of this, we use a proxy/wrapper around the generated lexer to handle everything correctly.

use super::generated::yarnspinnerlexer::{
    self, LocalTokenFactory, YarnSpinnerLexer as GeneratedYarnSpinnerLexer,
};
use crate::collections::*;
use crate::listeners::Diagnostic;
use crate::output::Position;
use crate::prelude::{create_common_token, DiagnosticSeverity, TokenExt};
use antlr_rust::token::CommonToken;
use antlr_rust::{
    char_stream::CharStream,
    token::{Token, TOKEN_DEFAULT_CHANNEL},
    token_factory::{CommonTokenFactory, TokenFactory},
    Lexer, TokenSource,
};
use std::cell::RefCell;
use std::ops::{Deref, DerefMut, Range};
use std::rc::Rc;

// To ensure we don't accidentally use the wrong lexer, this will produce errors on use.
#[allow(dead_code)]
type YarnSpinnerLexer = ();

antlr_rust::tid! { impl<'input, Input> TidAble<'input> for IndentAwareYarnSpinnerLexer<'input, Input> where Input:CharStream<From<'input>> }

/// A Lexer subclass that detects newlines and generates indent and dedent tokens accordingly.
///
/// ## Implementation notes
///
/// In contrast to the original implementation, the warnings emitted by this lexer are actually respected in the diagnostics.
pub struct IndentAwareYarnSpinnerLexer<
    'input,
    Input: CharStream<From<'input>>,
    TF: TokenFactory<'input> = LocalTokenFactory<'input>,
> {
    base: GeneratedYarnSpinnerLexer<'input, Input>,
    hit_eof: bool,
    /// Holds the last observed token from the stream.
    /// Used to see if a line is blank or not.
    last_token: Option<TF::Tok>,
    /// The collection of tokens that we have seen, but have not yet
    /// returned. This is needed when NextToken encounters a newline,
    /// which means we need to buffer indents or dedents. [`next_token`]
    /// only returns a single [`Token`] at a time, which
    /// means we use this list to buffer it.
    pending_tokens: Queue<TF::Tok>,
    /// A flag to say the last line observed was a shortcut or not.
    /// Used to determine if tracking indents needs to occur.
    line_contains_shortcut: bool,
    /// Keeps track of the last indentation encountered.
    /// This is used to see if depth has changed between lines.
    last_indent: isize,
    /// A stack keeping track of the levels of indentations we have seen so far that are relevant to shortcuts.
    unbalanced_indents: Stack<isize>,
    /// holds the line number of the last seen option.
    /// Lets us work out if the blank line needs to end the option.
    last_seen_option_content: Option<isize>,
    file_name: String,
    pub(crate) diagnostics: Rc<RefCell<Vec<Diagnostic>>>,
}

impl<'input, Input: CharStream<From<'input>>> Deref for IndentAwareYarnSpinnerLexer<'input, Input> {
    type Target = GeneratedYarnSpinnerLexer<'input, Input>;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl<'input, Input: CharStream<From<'input>>> DerefMut
    for IndentAwareYarnSpinnerLexer<'input, Input>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

impl<'input, Input: CharStream<From<'input>>> TokenSource<'input>
    for IndentAwareYarnSpinnerLexer<'input, Input>
{
    type TF = LocalTokenFactory<'input>;

    fn next_token(&mut self) -> <Self::TF as TokenFactory<'input>>::Tok {
        if self.hit_eof && !self.pending_tokens.0.is_empty() {
            // We have hit the EOF, but we have tokens still pending.
            // Start returning those tokens.
            self.pending_tokens.dequeue().unwrap()
        } else if self.base.input().size() == 0 {
            self.hit_eof = true;
            create_common_token(antlr_rust::token::TOKEN_EOF, "<EOF>")
        } else {
            // Get the next token, which will enqueue one or more new
            // tokens into the pending tokens queue.
            self.check_next_token();

            // `check_next_token` will always set at least one pending token if `self.base.input().size() > 0`
            // if `self.base.input().size() == 0`, the branch returning the EOF token is already entered ahead of this.
            self.pending_tokens.dequeue().unwrap()
        }
    }

    fn get_input_stream(&mut self) -> Option<&mut dyn antlr_rust::int_stream::IntStream> {
        self.base.get_input_stream()
    }

    fn get_source_name(&self) -> String {
        self.base.get_source_name()
    }

    fn get_token_factory(&self) -> &'input Self::TF {
        self.base.get_token_factory()
    }
}

/// Copied from generated/yarnspinnerlexer.rs
type From<'a> = <LocalTokenFactory<'a> as TokenFactory<'a>>::From;

impl<'input, Input: CharStream<From<'input>>> IndentAwareYarnSpinnerLexer<'input, Input>
where
    &'input LocalTokenFactory<'input>: Default,
{
    pub fn new(input: Input, file_name: String) -> Self {
        IndentAwareYarnSpinnerLexer {
            file_name,
            base: GeneratedYarnSpinnerLexer::new(input),
            hit_eof: false,
            last_token: Default::default(),
            pending_tokens: Default::default(),
            line_contains_shortcut: false,
            last_indent: Default::default(),
            unbalanced_indents: Default::default(),
            last_seen_option_content: None,
            diagnostics: Default::default(),
        }
    }

    fn check_next_token(&mut self) {
        let current = self.base.next_token();

        match current.token_type {
            // Insert indents or dedents depending on the next token's
            // indentation, and enqueues the newline at the correct place
            yarnspinnerlexer::NEWLINE => self.handle_newline_token(current.clone()),
            // Insert dedents before the end of the file, and then
            // enqueues the EOF.
            antlr_rust::token::TOKEN_EOF => self.handle_eof_token(current.clone()),
            yarnspinnerlexer::SHORTCUT_ARROW => {
                self.pending_tokens.enqueue(current.clone());
                self.line_contains_shortcut = true;
            }
            // we are at the end of the node
            // depth no longer matters
            // clear the stack
            yarnspinnerlexer::BODY_END => {
                self.line_contains_shortcut = false;
                self.last_indent = 0;
                self.unbalanced_indents.0.clear();
                self.last_seen_option_content = None;
                // [sic from the original!] TODO: this should be empty by now actually...
                self.pending_tokens.enqueue(current.clone());
            }
            _ => self.pending_tokens.enqueue(current.clone()),
        }

        // TODO: but... really?
        self.last_token = Some(current);
    }

    fn handle_newline_token(
        &mut self,
        current_token: Box<antlr_rust::token::GenericToken<std::borrow::Cow<'input, str>>>,
    ) {
        // We're about to go to a new line. Look ahead to see how indented it is.

        // insert the current NEWLINE token
        self.pending_tokens.enqueue(current_token.clone());

        if let Some(last_seen_option_content) = self.last_seen_option_content {
            // [sic!] we are a blank line
            if self
                .last_token
                .as_ref()
                .map(|last| current_token.token_type == last.token_type)
                .unwrap_or_default()
            {
                // is the option content directly above us?
                if self.base.get_line() - last_seen_option_content == 1 {
                    // [sic! (the whole thing)]
                    // so that we don't end up printing <ending option group> into the stream we set the text to be empty
                    // I dislike this and need to look into if you can set a debug text setting in ANTLR
                    // TODO: see above comment
                    // this.InsertToken("<ending option group>", YarnSpinnerLexer.BLANK_LINE_FOLLOWING_OPTION);
                    self.insert_token("", yarnspinnerlexer::BLANK_LINE_FOLLOWING_OPTION);
                }
                // disabling the option tracking
                self.last_seen_option_content = None;
            }
        }

        let current_indentation_length = self.get_length_of_newline_token(&current_token);

        // we need to actually see if there is a shortcut *somewhere* above us
        // if there isn't we just chug on without worrying
        if self.line_contains_shortcut {
            // we have a shortcut *somewhere* above us
            // that means we need to check our depth
            // and compare it to the shortcut depth

            // if the depth of the current line is greater than the previous one
            // we need to add this depth to the indents stack
            if current_indentation_length > self.last_indent {
                self.unbalanced_indents.push(current_indentation_length);
                // [sic!] so that we don't end up printing <indent to 8> into the stream we set the text to be empty
                // I dislike this and need to look into if you can set a debug text setting in ANTLR
                // TODO: see above comment
                // this.InsertToken($"<indent to {currentIndentationLength}>", YarnSpinnerLexer.INDENT);
                self.insert_token("", yarnspinnerlexer::INDENT);
            }

            // we've now started tracking the indentation, or ignored it, so can turn this off
            self.line_contains_shortcut = false;
            self.last_seen_option_content = Some(self.base.get_line());
        }

        // now we need to see if the current depth requires any indents or dedents
        // we do this by first checking to see if there are any unbalanced indents
        if let Some(&initial_top) = self.unbalanced_indents.peek() {
            // [sic!] later should make it check if indentation has changed inside the statement block and throw out a warning
            // this.warnings.Add(new Warning { Token = currentToken, Message = "Indentation inside of shortcut block has changed. This is generally a bad idea."});

            // while there are unbalanced indents
            // we need to check if the current line is shallower than the indent stack
            // if it is then we emit a dedent and continue checking

            let mut top = initial_top;

            while current_indentation_length < top {
                // so that we don't end up printing <indent from 8> into the stream we set the text to be empty
                // I dislike this and need to look into if you can set a debug text setting in ANTLR
                // TODO: see above comment
                // this.InsertToken($"<dedent from {top}>", YarnSpinnerLexer.DEDENT);
                self.insert_token("", yarnspinnerlexer::DEDENT);

                self.unbalanced_indents.pop();

                top = if let Some(&next) = self.unbalanced_indents.peek() {
                    next
                } else {
                    // we've dedented all the way out of the shortcut
                    // as such we are done with the option block
                    // previousLineWasOptionOrOptionBlock = false;
                    self.last_seen_option_content = Some(self.base.get_line());
                    0
                };
            }
        }

        // finally we update the last seen depth
        self.last_indent = current_indentation_length;
    }

    fn handle_eof_token(
        &mut self,
        current_token: Box<antlr_rust::token::GenericToken<std::borrow::Cow<'input, str>>>,
    ) {
        // We're at the end of the file. Emit as many dedents as we
        // currently have on the stack.
        while let Some(_indent) = self.unbalanced_indents.pop() {
            // so that we don't end up printing <dedent from 8> into the stream we set the text to be empty
            // I dislike this and need to look into if you can set a debug text setting in ANTLR
            // TODO: see above comment
            // this.InsertToken($"<dedent: {indent}>", YarnSpinnerLexer.DEDENT);
            self.insert_token("", yarnspinnerlexer::DEDENT);
        }

        // Finally, enqueue the EOF token.
        self.pending_tokens.enqueue(current_token);
    }

    /// Given a NEWLINE token, return the length of the indentation
    /// following it by counting the spaces and tabs after it.
    fn get_length_of_newline_token(
        &mut self,
        current_token: &antlr_rust::token::GenericToken<std::borrow::Cow<'input, str>>,
    ) -> isize {
        if current_token.token_type != yarnspinnerlexer::NEWLINE {
            panic!("Current token must NOT be newline")
        }

        let mut length = 0;
        let mut saw_spaces = false;
        let mut saw_tabs = false;

        for c in current_token.get_text().chars() {
            match c {
                ' ' => {
                    length += 1;
                    saw_spaces = true;
                }
                '\t' => {
                    length += 8; // Ye, really (see reference implementation)
                    saw_tabs = true;
                }
                _ => {}
            }
        }

        if saw_spaces && saw_tabs {
            self.diagnostics.borrow_mut().push(
                Diagnostic::from_message("Indentation contains tabs and spaces")
                    .with_range(get_newline_indentation_range(current_token))
                    .with_context(get_newline_indentation_text(current_token))
                    .with_start_line(current_token.line as usize)
                    .with_file_name(self.file_name.clone())
                    .with_severity(DiagnosticSeverity::Warning),
            );
        }

        length
    }

    /// Inserts a new token with the given text and type, as though it
    /// had appeared in the input stream.
    fn insert_token(&mut self, text: impl Into<String>, token_type: isize) {
        // https://www.antlr.org/api/Java/org/antlr/v4/runtime/Lexer.html#_tokenStartCharIndex
        let start_index = self.base.token_start_char_index + self.base.get_text().len() as isize;

        let line = self.get_line();
        let char_position_in_line = self.get_char_position_in_line();

        let token = CommonTokenFactory.create(
            self.base.input.as_mut(),
            token_type,
            Some(text.into()),
            TOKEN_DEFAULT_CHANNEL,
            start_index,
            start_index - 1,
            line,
            char_position_in_line,
        );

        self.pending_tokens.enqueue(token);
    }
}

fn get_newline_indentation_range(token: &CommonToken<'_>) -> Range<Position> {
    // +1 compared to similar code because we don't want to start at the newline
    let line = token.get_line_as_usize();

    let start = Position { line, character: 0 };
    let stop = Position {
        line,
        character: token.get_text().len(),
    };

    start..stop
}

fn get_newline_indentation_text(token: &CommonToken<'_>) -> String {
    // Skip newline
    token.get_text().chars().skip(1).collect()
}
