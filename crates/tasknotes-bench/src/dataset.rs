//! A deterministic dataset, written through the core's public operations so
//! every rule, index and Audit Entry is what real use produces. The same
//! seed and sizes give the same content in the same order; ids and timestamps
//! come from the core, as they do in real use.

use std::path::Path;

use serde_json::{Value, json};
use tasknotes_core::note_folders::CreateNoteFolder;
use tasknotes_core::notes::{CreateNote, SetNotePinned};
use tasknotes_core::privacy::SetNoteExcludeFromAi;
use tasknotes_core::removed::SetRemoved;
use tasknotes_core::tasks::{
    ChangeTask, CreateList, CreateTask, PatchTaskDetails, Priority, RecordProgress, TaskError,
    TaskStore, WorkStatus,
};

pub const SEED: u64 = 0x7461_736b_6e6f_7465;
pub const FULL_TASKS: usize = 50_000;
pub const FULL_NOTES: usize = 10_000;
const LISTS: usize = 40;
const FOLDERS: usize = 200;
const TAGS: usize = 60;

/// Words searched for, each in a known share of the records.
pub const TASK_COMMON: &str = "quarterly";
pub const TASK_MEDIUM: &str = "roadmap";
pub const TASK_RARE: &str = "zephyrine";
pub const PROGRESS_WORD: &str = "handoff";
pub const NOTE_COMMON: &str = "meeting";
pub const NOTE_RARE: &str = "kelvinite";

/// xorshift64*: small, stable, and independent of any crate's version.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn below(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }

    /// True in `per_mille` of a thousand calls.
    pub fn chance(&mut self, per_mille: usize) -> bool {
        self.below(1000) < per_mille
    }

    fn word(&mut self) -> String {
        const ONSETS: [&str; 12] = ["b", "d", "f", "k", "l", "m", "n", "p", "r", "s", "t", "v"];
        const VOWELS: [&str; 5] = ["a", "e", "i", "o", "u"];
        (0..2 + self.below(3))
            .map(|_| {
                let onset = ONSETS[self.below(ONSETS.len())];
                let vowel = VOWELS[self.below(VOWELS.len())];
                format!("{onset}{vowel}")
            })
            .collect()
    }

    pub fn words(&mut self, count: usize) -> String {
        (0..count)
            .map(|_| self.word())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// One Task's content, before it reaches the core.
pub struct TaskPlan {
    pub title: String,
    pub description: String,
    pub list: Option<usize>,
    pub change: Option<(WorkStatus, Priority)>,
    pub tags: Vec<String>,
    pub due_date: Option<String>,
    pub progress_note: Option<String>,
    pub removed: bool,
}

pub fn plan_task(rng: &mut Rng) -> TaskPlan {
    let title_words = 2 + rng.below(7);
    let mut title = rng.words(title_words);
    let long = rng.chance(50);
    let length = if long { 300 } else { 20 } + rng.below(if long { 200 } else { 60 });
    let mut description = rng.words(length);
    if rng.chance(200) {
        title.push(' ');
        title.push_str(TASK_COMMON);
    }
    if rng.chance(50) {
        description.push(' ');
        description.push_str(TASK_MEDIUM);
    }
    if rng.chance(1) {
        description.push(' ');
        description.push_str(TASK_RARE);
    }
    let list = rng.chance(700).then(|| rng.below(LISTS));
    let change = rng.chance(600).then(|| {
        let status = match rng.below(3) {
            0 => WorkStatus::Ready,
            1 => WorkStatus::InProgress,
            _ => WorkStatus::Completed,
        };
        let priority = match rng.below(5) {
            0 => Priority::VeryLow,
            1 => Priority::Low,
            2 => Priority::Medium,
            3 => Priority::High,
            _ => Priority::VeryHigh,
        };
        (status, priority)
    });
    let tags = if rng.chance(300) {
        (0..1 + rng.below(3))
            .map(|_| format!("tag{:02}", rng.below(TAGS)))
            .collect()
    } else {
        Vec::new()
    };
    let due_date = rng
        .chance(200)
        .then(|| format!("2027-{:02}-{:02}", 1 + rng.below(12), 1 + rng.below(28)));
    let progress_note = rng.chance(100).then(|| {
        let count = 10 + rng.below(30);
        format!("{PROGRESS_WORD} {}", rng.words(count))
    });
    let removed = rng.chance(10);
    TaskPlan {
        title,
        description,
        list,
        change,
        tags,
        due_date,
        progress_note,
        removed,
    }
}

/// One Note's content, before it reaches the core.
pub struct NotePlan {
    pub markdown: String,
    pub folder: Option<usize>,
    pub pinned: bool,
    pub excluded: bool,
}

pub fn plan_note(rng: &mut Rng) -> NotePlan {
    let heading_words = 2 + rng.below(6);
    let heading = rng.words(heading_words);
    let long = rng.chance(30);
    let length = if long { 2000 } else { 50 } + rng.below(if long { 1000 } else { 550 });
    let mut body = rng.words(length);
    if rng.chance(300) {
        body.push(' ');
        body.push_str(NOTE_COMMON);
    }
    if rng.chance(2) {
        body.push(' ');
        body.push_str(NOTE_RARE);
    }
    NotePlan {
        markdown: format!("# {heading}\n\n{body}\n"),
        folder: rng.chance(700).then(|| rng.below(FOLDERS)),
        pinned: rng.chance(5),
        excluded: rng.chance(30),
    }
}

/// A fingerprint of the planned content: equal when the dataset is the same.
pub fn fingerprint(tasks: usize, notes: usize) -> u64 {
    let mut rng = Rng::new(SEED);
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |text: &str| {
        for byte in text.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
        }
    };
    for _ in 0..tasks {
        let plan = plan_task(&mut rng);
        mix(&plan.title);
        mix(&plan.description);
    }
    for _ in 0..notes {
        mix(&plan_note(&mut rng).markdown);
    }
    hash
}

/// What `measure` needs to name: one List, one folder and one tag that exist.
pub fn seed(dir: &Path, tasks: usize, notes: usize) -> Result<Value, TaskError> {
    std::fs::create_dir_all(dir)?;
    let store = TaskStore::open(&dir.join("tasknotes.db"))?;
    let mut rng = Rng::new(SEED);
    let lists = (0..LISTS)
        .map(|i| {
            store
                .create_list(CreateList {
                    name: format!("Synthetic list {i:02}"),
                })
                .map(|list| list.id)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let folders = (0..FOLDERS)
        .map(|i| {
            store
                .create_note_folder(CreateNoteFolder {
                    name: format!("Synthetic folder {i:03}"),
                    parent_id: None,
                })
                .map(|folder| folder.id)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut first_tasks = Vec::new();
    for _ in 0..tasks {
        let plan = plan_task(&mut rng);
        let id = write_task(&store, &lists, plan)?;
        if first_tasks.len() < 2000 {
            first_tasks.push(id);
        }
    }
    let mut first_notes = Vec::new();
    for _ in 0..notes {
        let plan = plan_note(&mut rng);
        let excluded = plan.excluded;
        let id = write_note(&store, &folders, plan)?;
        if !excluded && first_notes.len() < 2000 {
            first_notes.push(id);
        }
    }
    let mut linked = 0;
    for (task, note) in first_tasks.iter().zip(&first_notes) {
        // A Removed Task refuses a link; the rest make up the links measured.
        if store.link_note(task, note).is_ok() {
            linked += 1;
        }
    }
    Ok(json!({
        "seed": SEED,
        "tasks": tasks,
        "notes": notes,
        "links": linked,
        "fingerprint": format!("{:016x}", fingerprint(tasks, notes)),
        "list": lists[0],
        "folder": folders[0],
        "tag": "tag07",
    }))
}

fn write_task(store: &TaskStore, lists: &[String], plan: TaskPlan) -> Result<String, TaskError> {
    let list = plan.list.map(|index| lists[index].as_str());
    let mut task = store.create_task_in_list(
        CreateTask {
            title: plan.title,
            description: plan.description,
        },
        list,
    )?;
    if let Some((status, priority)) = plan.change {
        // Ready at the default priority is no change, which the core refuses.
        match store.change_task(ChangeTask {
            id: task.id.clone(),
            expected_revision: task.revision,
            status: Some(status),
            priority: Some(priority),
            progress: None,
        }) {
            Ok(changed) => task = changed,
            Err(TaskError::EmptyProgressUpdate) => {}
            Err(error) => return Err(error),
        }
    }
    if !plan.tags.is_empty() || plan.due_date.is_some() {
        task = store.patch_task_details(PatchTaskDetails {
            id: task.id,
            expected_revision: task.revision,
            tags: (!plan.tags.is_empty()).then_some(plan.tags),
            due_date: plan.due_date.map(Some),
        })?;
    }
    if let Some(note) = plan.progress_note {
        task = store.record_progress(RecordProgress {
            id: task.id,
            expected_revision: task.revision,
            status: None,
            progress: None,
            next_step: None,
            note: Some(note),
        })?;
    }
    let id = task.id.clone();
    if plan.removed {
        store.set_task_removed(SetRemoved {
            id: task.id,
            expected_revision: task.revision,
            removed: true,
        })?;
    }
    Ok(id)
}

fn write_note(store: &TaskStore, folders: &[String], plan: NotePlan) -> Result<String, TaskError> {
    let note = store.create_note(CreateNote {
        markdown: plan.markdown,
        folder_id: plan.folder.map(|index| folders[index].clone()),
    })?;
    if plan.pinned {
        store.set_note_pinned(SetNotePinned {
            id: note.id.clone(),
            pinned: true,
        })?;
    }
    if plan.excluded {
        store.set_note_exclude_from_ai(SetNoteExcludeFromAi {
            id: note.id.clone(),
            exclude_from_ai: true,
        })?;
    }
    Ok(note.id)
}
