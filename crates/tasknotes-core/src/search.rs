use rusqlite::{Transaction, params_from_iter, types::Value};
use serde::Deserialize;
use uuid::Uuid;

use crate::tasks::{
    COLUMNS, Difficulty, DifficultyFilter, Priority, SortMode, TaskError, TaskPage, TaskStore,
    TaskView, WorkStatus, read_task,
};

/// Whether a query looks at active Tasks or only at Removed ones (ADR-0002).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskScope {
    #[default]
    Active,
    Removed,
}

/// Every Task listing goes through this one query: search text, filters,
/// scope and sort. Work-status groups always stay In Progress → Ready → Completed.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TaskQuery {
    pub text: String,
    pub status: Option<WorkStatus>,
    pub priority: Option<Priority>,
    /// `None` is Any Difficulty: no filter.
    pub difficulty: Option<DifficultyFilter>,
    pub tag: Option<String>,
    pub view: TaskView,
    pub scope: TaskScope,
    pub sort: SortMode,
    /// Narrow the query to one Task, to ask whether it matches the rest.
    pub task_id: Option<String>,
}

const MAX_SEARCH_WORDS: usize = 16;

const STATUS_RANK: &str = "CASE status WHEN 'in_progress' THEN 0 WHEN 'ready' THEN 1 ELSE 2 END";
const PRIORITY_RANK: &str = "CASE priority WHEN 'very_high' THEN 5 WHEN 'high' THEN 4 WHEN 'medium' THEN 3 WHEN 'low' THEN 2 ELSE 1 END";
/// Must match the v17 index expression and `Difficulty::rank`; the tests
/// below check both.
const DIFFICULTY_RANK: &str =
    "CASE difficulty WHEN 'very_hard' THEN 5 WHEN 'hard' THEN 4 WHEN 'medium' THEN 3
               WHEN 'easy' THEN 2 WHEN 'very_easy' THEN 1 ELSE 0 END";
const TOKENIZER: &str = "tokenize='unicode61 remove_diacritics 2'";

/// Schema v5: the Task Removed disposition plus FTS5 indexes kept in sync by
/// triggers, so every writer of this SQLite file maintains them.
pub(crate) fn migrate_v5(migration: &Transaction) -> Result<(), TaskError> {
    migration.execute_batch(&format!(
        "ALTER TABLE tasks ADD COLUMN removed_at INTEGER;
         CREATE VIRTUAL TABLE task_fields_fts USING fts5(title, description, next_step, {TOKENIZER});
         CREATE VIRTUAL TABLE progress_fts USING fts5(note, next_step, task_rowid UNINDEXED, {TOKENIZER});
         INSERT INTO task_fields_fts(rowid, title, description, next_step)
           SELECT rowid, title, description, next_step FROM tasks;
         INSERT INTO progress_fts(rowid, note, next_step, task_rowid)
           SELECT p.sequence, p.note, p.next_step, t.rowid
           FROM progress_entries p JOIN tasks t ON t.id = p.task_id
           WHERE p.note <> '' OR p.next_step <> '';
         CREATE TRIGGER tasks_fts_insert AFTER INSERT ON tasks BEGIN
           INSERT INTO task_fields_fts(rowid, title, description, next_step)
           VALUES (new.rowid, new.title, new.description, new.next_step);
         END;
         CREATE TRIGGER tasks_fts_update AFTER UPDATE OF title, description, next_step ON tasks BEGIN
           DELETE FROM task_fields_fts WHERE rowid = old.rowid;
           INSERT INTO task_fields_fts(rowid, title, description, next_step)
           VALUES (new.rowid, new.title, new.description, new.next_step);
         END;
         CREATE TRIGGER tasks_fts_delete AFTER DELETE ON tasks BEGIN
           DELETE FROM task_fields_fts WHERE rowid = old.rowid;
         END;
         CREATE TRIGGER progress_fts_insert AFTER INSERT ON progress_entries
           WHEN new.note <> '' OR new.next_step <> '' BEGIN
           INSERT INTO progress_fts(rowid, note, next_step, task_rowid)
           VALUES (new.sequence, new.note, new.next_step,
                   (SELECT rowid FROM tasks WHERE id = new.task_id));
         END;
         PRAGMA user_version = 5;"
    ))?;
    Ok(())
}

/// Split user text into plain words. Each becomes a quoted prefix term, so the
/// input can never reach FTS5 query syntax. Extra words are refused rather than
/// dropped, so a result never silently ignores part of the search.
pub(crate) fn search_terms(text: &str) -> Result<Vec<String>, TaskError> {
    let terms: Vec<_> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| format!("\"{word}\"*"))
        .collect();
    if terms.len() > MAX_SEARCH_WORDS {
        return Err(TaskError::TooManySearchWords);
    }
    Ok(terms)
}

/// A `version|time|id` keyset cursor, as the Removed view and link lists
/// hand out; anything else is `invalid`.
pub(crate) fn parse_time_id_cursor(
    value: &str,
    version: &str,
    invalid: TaskError,
) -> Result<(i64, String), TaskError> {
    let mut fields = value.split('|');
    match (fields.next(), fields.next(), fields.next(), fields.next()) {
        (Some(v), Some(time), Some(id), None) if v == version && Uuid::parse_str(id).is_ok() => {
            match time.parse() {
                Ok(time) => Ok((time, id.to_owned())),
                Err(_) => Err(invalid),
            }
        }
        _ => Err(invalid),
    }
}

/// FNV-1a: binds a cursor to the exact query that produced it.
pub(crate) fn fingerprint(parts: &[&str]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in parts.join("\u{0}").bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

struct After {
    status: i64,
    /// The Priority or Difficulty rank of the sort; 0 under Newest.
    rank: i64,
    time: i64,
    id: String,
}

fn parse_cursor(value: &str, key: &str, sort: SortMode) -> Result<After, TaskError> {
    let fields: Vec<_> = value.split('|').collect();
    if fields.len() != 6 || fields[0] != "v4" || fields[1] != key {
        return Err(TaskError::InvalidCursor);
    }
    let number = |text: &str| text.parse::<i64>().map_err(|_| TaskError::InvalidCursor);
    let after = After {
        status: number(fields[2])?,
        rank: number(fields[3])?,
        time: number(fields[4])?,
        id: fields[5].to_owned(),
    };
    if !(0..=2).contains(&after.status)
        || !match sort {
            SortMode::Priority => (1..=5).contains(&after.rank),
            SortMode::Difficulty => (0..=5).contains(&after.rank),
            SortMode::Newest => after.rank == 0,
        }
        || after.time < 0
        || Uuid::parse_str(&after.id).is_err()
    {
        return Err(TaskError::InvalidCursor);
    }
    Ok(after)
}

impl TaskStore {
    /// Bounded keyset page over one query. A cursor is valid only for the
    /// query that produced it, so pages never repeat or skip across queries.
    pub fn query_tasks(
        &self,
        query: &TaskQuery,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<TaskPage, TaskError> {
        if !(1..=100).contains(&limit) {
            return Err(TaskError::InvalidPageSize);
        }
        let terms = search_terms(&query.text)?;
        let tag_fold = query
            .tag
            .as_deref()
            .map(|tag| crate::organization::tag_fold(tag.trim()));
        let sort = query.sort;
        let key = fingerprint(&[
            sort.as_cursor(),
            &query.view.cursor_key(),
            match query.scope {
                TaskScope::Active => "active",
                TaskScope::Removed => "removed",
            },
            query.status.map_or("", WorkStatus::as_sql),
            query.priority.map_or("", Priority::as_sql),
            query.difficulty.map_or("", DifficultyFilter::as_name),
            tag_fold.as_deref().unwrap_or(""),
            &terms.join(" "),
            query.task_id.as_deref().unwrap_or(""),
        ]);
        let after = cursor
            .map(|value| parse_cursor(value, &key, sort))
            .transpose()?;

        let (sql, values) = page_sql(query, terms, tag_fold, after.as_ref(), limit);

        let conn = self.connect()?;
        if let (TaskScope::Active, TaskView::List(id)) = (query.scope, &query.view) {
            crate::organization::require_active_list(&conn, id)?;
        }
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params_from_iter(values))?;
        let mut items = Vec::with_capacity(limit as usize + 1);
        while let Some(row) = rows.next()? {
            items.push(read_task(row)?);
        }
        drop(rows);
        drop(stmt);
        crate::organization::hydrate_tags(&conn, &mut items)?;

        let next_cursor = if items.len() > limit as usize {
            items.pop();
            items.last().map(|task| {
                let rank = match sort {
                    SortMode::Priority => task.priority.rank(),
                    SortMode::Difficulty => Difficulty::rank(task.difficulty),
                    SortMode::Newest => 0,
                };
                format!(
                    "v4|{key}|{}|{rank}|{}|{}",
                    task.status.rank(),
                    task.created_at,
                    task.id
                )
            })
        } else {
            None
        };
        Ok(TaskPage { items, next_cursor })
    }
}

/// The SQL of one page of a query, and its bound values.
fn page_sql(
    query: &TaskQuery,
    terms: Vec<String>,
    tag_fold: Option<String>,
    after: Option<&After>,
    limit: u32,
) -> (String, Vec<Value>) {
    let sort = query.sort;
    let mut values: Vec<Value> = Vec::new();
    let mut bind = |value: Value| {
        values.push(value);
        format!("?{}", values.len())
    };

    let mut filters = vec![match query.scope {
        TaskScope::Active => task_visible!("tasks").to_owned(),
        TaskScope::Removed => "tasks.removed_at IS NOT NULL".to_owned(),
    }];
    match &query.view {
        TaskView::All => {}
        TaskView::Inbox => filters.push("tasks.list_id IS NULL".to_owned()),
        TaskView::List(id) => {
            filters.push(format!("tasks.list_id = {}", bind(Value::Text(id.clone()))))
        }
    }
    if let Some(id) = &query.task_id {
        filters.push(format!("tasks.id = {}", bind(Value::Text(id.clone()))));
    }
    if let Some(status) = query.status {
        filters.push(format!(
            "status = {}",
            bind(Value::Text(status.as_sql().into()))
        ));
    }
    if let Some(priority) = query.priority {
        filters.push(format!(
            "priority = {}",
            bind(Value::Text(priority.as_sql().into()))
        ));
    }
    match query.difficulty {
        None => {}
        Some(DifficultyFilter::NoDifficulty) => filters.push("difficulty IS NULL".to_owned()),
        Some(DifficultyFilter::Is(difficulty)) => filters.push(format!(
            "difficulty = {}",
            bind(Value::Text(difficulty.as_sql().into()))
        )),
    }
    if let Some(fold) = tag_fold {
        filters.push(format!(
            "EXISTS (SELECT 1 FROM task_tags tt JOIN tags t ON t.name = tt.tag_name
               WHERE tt.task_id = tasks.id AND t.fold = {})",
            bind(Value::Text(fold))
        ));
    }
    // Every word must match the Task's fields or any of its Progress Entries.
    for term in terms {
        let term = bind(Value::Text(term));
        filters.push(format!(
            "(tasks.rowid IN (SELECT rowid FROM task_fields_fts WHERE task_fields_fts MATCH {term})
              OR tasks.rowid IN (SELECT task_rowid FROM progress_fts WHERE progress_fts MATCH {term}))"
        ));
    }

    let rank_expr = match sort {
        SortMode::Priority => PRIORITY_RANK,
        SortMode::Difficulty => DIFFICULTY_RANK,
        SortMode::Newest => "0",
    };
    let secondary_order = if sort == SortMode::Newest {
        ""
    } else {
        "sort_rank DESC, "
    };
    let cursor_filter = match after {
        None => "1".to_owned(),
        Some(after) => {
            let status = bind(Value::Integer(after.status));
            let time = bind(Value::Integer(after.time));
            let id = bind(Value::Text(after.id.clone()));
            if sort == SortMode::Newest {
                format!(
                    "(status_rank > {status}
                      OR (status_rank = {status} AND created_at < {time})
                      OR (status_rank = {status} AND created_at = {time} AND id < {id}))"
                )
            } else {
                let rank = bind(Value::Integer(after.rank));
                format!(
                    "(status_rank > {status}
                      OR (status_rank = {status} AND sort_rank < {rank})
                      OR (status_rank = {status} AND sort_rank = {rank} AND created_at < {time})
                      OR (status_rank = {status} AND sort_rank = {rank}
                          AND created_at = {time} AND id < {id}))"
                )
            }
        }
    };
    let page_size = bind(Value::Integer(i64::from(limit) + 1));
    // Do not add a constant sort_rank sort key in Newest mode: doing so
    // forces SQLite to build a temporary B-tree instead of using the index.
    let sql = format!(
        "WITH ranked AS (
            SELECT {COLUMNS}, {STATUS_RANK} AS status_rank, {rank_expr} AS sort_rank
            FROM tasks WHERE {}
         )
         SELECT {COLUMNS} FROM ranked
         WHERE {cursor_filter}
         ORDER BY status_rank ASC, {secondary_order}created_at DESC, id DESC
         LIMIT {page_size}",
        filters.join(" AND ")
    );
    (sql, values)
}

#[cfg(test)]
mod difficulty_rank_tests {
    use super::*;

    fn squeeze(sql: &str) -> String {
        sql.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// `DIFFICULTY_RANK`, `Difficulty::rank` (cursors) and the v17 index
    /// expression rank alike, so they cannot drift apart unnoticed.
    #[test]
    fn the_difficulty_rank_agrees_with_difficulty_rank_and_the_v17_index() {
        let dir = tempfile::tempdir().unwrap();
        let store = TaskStore::open(&dir.path().join("t.db")).unwrap();
        let conn = store.connect().unwrap();

        let levels = std::iter::once(None).chain(Difficulty::ALL.into_iter().map(Some));
        for difficulty in levels {
            let rank: i64 = conn
                .query_row(
                    &format!("SELECT {DIFFICULTY_RANK} FROM (SELECT ?1 AS difficulty)"),
                    [difficulty.map(Difficulty::as_sql)],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(rank, Difficulty::rank(difficulty), "{difficulty:?}");
        }

        let index: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name = 'tasks_status_difficulty_order'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            squeeze(&index).contains(&squeeze(DIFFICULTY_RANK)),
            "{index}"
        );

        // The Difficulty sort is served by the index, with no sort of its own.
        for after in [
            None,
            Some(After {
                status: 1,
                rank: 3,
                time: 1,
                id: Uuid::nil().to_string(),
            }),
        ] {
            let query = TaskQuery {
                sort: SortMode::Difficulty,
                ..TaskQuery::default()
            };
            let (sql, values) = page_sql(&query, Vec::new(), None, after.as_ref(), 50);
            let mut statement = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
            let plan: Vec<String> = statement
                .query_map(params_from_iter(values), |row| row.get::<_, String>(3))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap();
            let plan = plan.join("\n");
            assert!(plan.contains("tasks_status_difficulty_order"), "{plan}");
            assert!(!plan.contains("TEMP B-TREE"), "{plan}");
        }
    }
}
