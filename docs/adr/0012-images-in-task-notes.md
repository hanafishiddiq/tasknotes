# Images in a Task's notes are kept in the database

Status: accepted (2026-11-18, batch 4)

The owner asked that a Task's notes can hold images, placed between the text where they were put, with no image on the collapsed row. A Task's notes therefore use the Note editor (ADR 0003) and stay Markdown in `tasks.description`; an image is a reference in that Markdown, `![image](tasknotes-image:ID)`, and its bytes live in their own table, `task_images` (schema v26).

We keep the bytes in SQLite rather than in files beside it so that the daily, manual and pre-restore backups, and a restore, carry every image with no second store to keep in step (ADR 0005's backups copy the database file). Only PNG, JPEG, GIF and WebP are kept, checked by their own first bytes; SVG is refused because it can carry scripts. One image is at most 10 MB. Images travel between the window and the core as raw bytes (the Tauri raw request body and response), never as JSON numbers, and the window draws them from `blob:` URLs, which the content security policy now allows for images only. The window takes file drops itself (`dragDropEnabled: false`), so dropping an image on the notes reaches the editor.

An image is never deleted: removing its reference from the notes leaves the image in the database, as nothing in Tasknotes is purged (ADR 0002). Adding one is audited (`task.image.add`); the Task itself is not written until its notes are saved with the reference. A Removed Task, or one in a Removed List, takes no new images and keeps showing its own.

The CLI and MCP see the reference in the description and are told to keep it as it is; they cannot see the image. JSON export carries each image in Base64 (`task_images`); Markdown export writes them to `Tasks/images/` and points the references at those files.

Notes written as plain text before (by the old field, the CLI or MCP) are read as Markdown; the editor writes them back with Markdown's escapes (`snake\_case`), so a draft equal to the saved notes as the editor writes them counts as no edit, and the collapsed row shows the notes without those escapes. The notes cannot be saved while an image is still on its way to the core. A file dropped anywhere but a Task's notes is ignored, so the webview never opens it in place of the app.
