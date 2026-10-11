import { $, browser, expect } from "@wdio/globals";

describe("Tasknotes native end-to-end: actual process restart", () => {
  it("reloads persisted Task, previous incomplete progress, Next Step and dated journal", async () => {
    await browser.switchToWindow("main");
    const row = await $('article[aria-label="QA-WDIO-EDITED"]');
    await row.waitForExist({ timeout: 20000 });
    await expect($('article[aria-label="QA-WDIO-EDITED"] .status-pill')).toHaveText("In Progress");
    await expect($('article[aria-label="QA-WDIO-EDITED"] .priority-pill')).toHaveText("Very high");
    await expect($('article[aria-label="QA-WDIO-EDITED"] .progress-readout')).toHaveText("60%");

    await $('button[aria-label="Open QA-WDIO-EDITED"]').click();
    await expect($('textarea[aria-label="Edit description"]')).toHaveValue(
      "Edited via actual native WebDriver",
    );
    await expect($('textarea[aria-label="Next step"]')).toHaveValue("Verify automated restart");
    const history = await $('section[aria-label="Progress history"]');
    await browser.waitUntil(async () => (await history.getText()).includes("Native acceptance passed"), {
      timeout: 10000,
      timeoutMsg: "Persisted progress note missing after native process restart",
    });
  });
});
