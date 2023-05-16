//! Adapted from <https://github.com/YarnSpinnerTool/YarnSpinner/blob/da39c7195107d8211f21c263e4084f773b84eaff/YarnSpinner/Dialogue.cs>, which we split off into multiple files
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};
use yarn_slinger_core::prelude::*;

/// A trait for providing text to a [`Dialogue`](crate::prelude::Dialogue).
///
/// ## Implementation notes
///
/// By injecting this, we don't need to expose `Dialogue.ExpandSubstitutions` and `Dialogue.ParseMarkup`, since we can apply them internally.
pub trait TextProvider: Debug + Send + Sync {
    fn clone_shallow(&self) -> Box<dyn TextProvider + Send + Sync>;
    fn get_text(&self, id: &LineId) -> Option<String>;
    fn set_language_code(&mut self, language_code: String);
}

impl Clone for Box<dyn TextProvider + Send + Sync> {
    fn clone(&self) -> Self {
        self.clone_shallow()
    }
}

/// A basic implementation of [`TextProvider`] that uses a [`HashMap`] to store the text.
#[derive(Debug, Clone, Default)]
pub struct StringTableTextProvider {
    string_table: Arc<RwLock<HashMap<LineId, String>>>,
    language_code: Arc<RwLock<Option<String>>>,
}

impl StringTableTextProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_string_table(string_table: HashMap<LineId, String>) -> Self {
        Self {
            string_table: Arc::new(RwLock::new(string_table)),
            language_code: Arc::new(RwLock::new(None)),
        }
    }

    pub fn set_string_table(&mut self, string_table: HashMap<LineId, String>) {
        *self.string_table.write().unwrap() = string_table;
    }
}

impl TextProvider for StringTableTextProvider {
    fn clone_shallow(&self) -> Box<dyn TextProvider + Send + Sync> {
        Box::new(self.clone())
    }

    fn get_text(&self, id: &LineId) -> Option<String> {
        self.string_table.read().unwrap().get(id).cloned()
    }

    fn set_language_code(&mut self, language_code: String) {
        self.language_code.write().unwrap().replace(language_code);
    }
}
