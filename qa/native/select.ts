import { $, browser } from "@wdio/globals";

/**
 * WKWebView's test-only embedded WebDriver may fail to commit a hidden native
 * select when selectByAttribute is used. Fall back to a scoped DOM change
 * event inside this isolated Tasknotes WKWebView, never OS-level input.
 *
 * Assert the real WebView value AND downstream Rust/SQLite persistence;
 * DOM-only state changes are insufficient to pass E2E acceptance.
 */
export async function selectOption(selector: string, value: string): Promise<void> {
  const el = await $(selector);
  await el.selectByAttribute("value", value);
  if (await el.getValue() !== value) {
    console.log("[Tasknotes QA] WKWebView select fallback:", selector, value);
    await browser.execute((target, expected) => {
      const node = document.querySelector<HTMLSelectElement>(target);
      if (!node) throw new Error("Missing native test select " + target);
      node.value = expected;
      node.dispatchEvent(new Event("input", { bubbles: true }));
      node.dispatchEvent(new Event("change", { bubbles: true }));
    }, selector, value);
  }
  await browser.waitUntil(async () => (await $(selector).getValue()) === value, {
    timeout: 10000,
    timeoutMsg: "WKWebView never selected " + value,
  });
}
