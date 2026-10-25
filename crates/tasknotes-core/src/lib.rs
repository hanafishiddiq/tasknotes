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

pub mod links;
pub mod note_folders;
pub mod notes;
mod organization;
pub mod removed;
mod search;
pub mod tasks;
mod text_fold;
