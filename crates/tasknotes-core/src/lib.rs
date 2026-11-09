/// The ids of every Note Folder hidden by Removed: each Removed folder and
/// every folder below one. Folders are few, so this one recursive query is
/// cheaper than walking each Note's ancestors. `UNION` stops on a damaged,
/// cyclic tree.
macro_rules! hidden_folders {
    () => {
        "WITH RECURSIVE hidden(id) AS (
            SELECT id FROM note_folders WHERE removed_at IS NOT NULL
            UNION
            SELECT f.id FROM note_folders f JOIN hidden h ON f.parent_id = h.id
         ) SELECT id FROM hidden"
    };
}

/// SQL: the Task aliased `$t` sits in a List that is in Removed.
macro_rules! task_in_removed_list {
    ($t:literal) => {
        concat!(
            "EXISTS (SELECT 1 FROM lists WHERE lists.id = ",
            $t,
            ".list_id AND lists.removed_at IS NOT NULL)"
        )
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
pub mod export;
pub mod links;
mod manual_order;
pub mod note_folders;
pub mod notes;
mod organization;
pub mod privacy;
pub mod removed;
mod search;
pub mod tasks;
mod text_fold;
