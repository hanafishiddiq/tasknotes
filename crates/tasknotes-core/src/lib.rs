/// The ids of every row of `$table` (`lists` or `note_folders`) hidden by
/// Removed: each Removed one and every one below it (ADR 0002, ADR 0009).
/// Containers are few, so this one recursive query is cheaper than walking
/// each item's ancestors. `UNION` stops on a damaged, cyclic tree.
macro_rules! hidden_containers {
    ($table:literal) => {
        concat!(
            "WITH RECURSIVE hidden(id) AS (
            SELECT id FROM ",
            $table,
            " WHERE removed_at IS NOT NULL
            UNION
            SELECT c.id FROM ",
            $table,
            " c JOIN hidden h ON c.parent_id = h.id
         ) SELECT id FROM hidden"
        )
    };
}

/// The ids of every Note Folder hidden by Removed.
macro_rules! hidden_folders {
    () => {
        hidden_containers!("note_folders")
    };
}

/// The ids of every List hidden by Removed: Removed itself or inside one.
macro_rules! hidden_lists {
    () => {
        hidden_containers!("lists")
    };
}

/// SQL: the Task aliased `$t` sits in a List that is in Removed, or inside
/// one.
macro_rules! task_in_removed_list {
    ($t:literal) => {
        concat!("coalesce(", $t, ".list_id IN (", hidden_lists!(), "), 0)")
    };
}

/// SQL: the Task aliased `$t` is shown (ADR-0002): not in Removed itself and
/// not hidden by a Removed List. The one rule every view, search and link
/// list uses.
macro_rules! task_visible {
    ($t:literal) => {
        concat!(
            "(",
            $t,
            ".removed_at IS NULL AND NOT ",
            task_in_removed_list!($t),
            ")"
        )
    };
}

/// SQL: the Note aliased `$n` sits in a Removed folder or below one.
macro_rules! note_in_hidden_folder {
    ($n:literal) => {
        concat!(
            "coalesce(",
            $n,
            ".folder_id IN (",
            hidden_folders!(),
            "), 0)"
        )
    };
}

/// SQL: the Note aliased `$n` is shown: not in Removed itself and not hidden
/// by a Removed folder above it.
macro_rules! note_visible {
    ($n:literal) => {
        concat!(
            "(",
            $n,
            ".removed_at IS NULL AND NOT ",
            note_in_hidden_folder!($n),
            ")"
        )
    };
}

pub mod ai_access;
pub mod audit;
pub mod backup;
pub mod bulk;
pub mod calendar;
mod custom_order;
pub mod export;
pub mod links;
mod manual_order;
pub mod note_folders;
pub mod notes;
mod organization;
pub mod privacy;
pub mod removed;
mod search;
pub mod style;
pub mod tasks;
mod text_fold;
pub mod tree;
