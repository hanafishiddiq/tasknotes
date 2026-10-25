//! How names typed by people are compared: Tags and Note Folder names.

use caseless::Caseless;
use unicode_normalization::UnicodeNormalization;

/// A name's identity: canonically decomposed, fully case-folded (ß is ss, final
/// and medial sigma are one letter), then composed again. "Straße" and
/// "STRASSE", or a precomposed é and e with a combining accent, fold alike.
/// SQLite NOCASE folds only ASCII, so the folded form is stored and compared.
pub(crate) fn fold(name: &str) -> String {
    name.nfd().default_case_fold().nfc().collect()
}
