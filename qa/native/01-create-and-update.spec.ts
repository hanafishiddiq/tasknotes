import { $, browser, expect } from "@wdio/globals";
import { selectOption } from "./select";

describe("Tasknotes native end-to-end: real Tauri IPC", () => {
  it("creates, edits, and journals a Task without mouse/screen injection", async () => {
    await browser.switchToWindow("main");
    const title = await $('input[aria-label="Task title"]');
    await title.waitForExist({ timeout: 20000 });
    await title.setValue("QA-WDIO-CREATED");
    await $('textarea[aria-label="Description (optional)"]').setValue("Created via native WDIO");
    await $('button=Add task').click();

    await $('article[aria-label="QA-WDIO-CREATED"]').waitForExist({ timeout: 10000 });
    await $('button[aria-label="Open QA-WDIO-CREATED"]').click();
    await $('input[aria-label="Edit title"]').setValue("QA-WDIO-EDITED");
    await $('textarea[aria-label="Edit description"]').setValue("Edited via actual native WebDriver");
    await $('button=Save changes').click();

    await $('article[aria-label="QA-WDIO-EDITED"]').waitForExist({ timeout: 10000 });
    await $('button[aria-label="Open QA-WDIO-EDITED"]').click();

    await selectOption('select[aria-label="Priority"]', "very_high");
    await expect($('article[aria-label="QA-WDIO-EDITED"] .priority-pill')).toHaveText("Very high");

    await selectOption('select[aria-label="Work status"]', "in_progress");
    await expect($('article[aria-label="QA-WDIO-EDITED"] .status-pill')).toHaveText("In Progress");

    await $('input[aria-label="Progress percentage"]').setValue("60");
    await $('textarea[aria-label="Next step"]').setValue("Verify automated restart");
    await $('textarea[aria-label="Progress note"]').setValue("Native acceptance passed");
    await $('button=Save progress update').click();

    await expect($('article[aria-label="QA-WDIO-EDITED"] .progress-readout')).toHaveText("60%");
    const history = await $('section[aria-label="Progress history"]');
    await browser.waitUntil(async () => (await history.getText()).includes("Native acceptance passed"), {
      timeout: 15000,
      timeoutMsg: "Native progress note was not visible in real WebView history",
    });

    await selectOption('select[aria-label="Work status"]', "completed");
    await expect($('article[aria-label="QA-WDIO-EDITED"] .progress-readout')).toHaveText("100%");

    await selectOption('select[aria-label="Work status"]', "in_progress");
    await expect($('article[aria-label="QA-WDIO-EDITED"] .progress-readout')).toHaveText("60%");

    const sort = await $('select[aria-label="Sort tasks"]');
    await selectOption('select[aria-label="Sort tasks"]', "newest");
    await expect(sort).toHaveValue("newest");
    await browser.waitUntil(async () => await sort.isEnabled(), { timeout: 10000 });
    await selectOption('select[aria-label="Sort tasks"]', "priority");
    await expect(sort).toHaveValue("priority");
  });
});
