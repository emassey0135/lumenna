// Playwright's test, with WebKit given a persistent context for each test: an ephemeral WebKit
// context is a private window, which refuses the origin private file system the store lives in
// ("An error occurred while getting the directory handle"), as Safari's own private windows do.
// WebKit's persistent contexts share one origin's storage whatever folder each is given, so
// each test starts by emptying it, and WebKit's tests run one at a time (`--workers=1`).
// Chromium and Firefox keep Playwright's ephemeral contexts.

import { test as base } from "@playwright/test";
import type { Browser, BrowserContext } from "@playwright/test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

export { expect } from "@playwright/test";
export type { Page } from "@playwright/test";

type Fixtures = {
  /** Opens another browser context, as another browser on another device would be. */
  freshContext: () => Promise<BrowserContext>;
};

/** A persistent WebKit context with this origin's storage emptied, as a new browser has it. */
async function emptyWebKit(
  playwright: { webkit: { launchPersistentContext: (dir: string, options: object) => Promise<BrowserContext> } },
  baseURL: string | undefined,
  viewport: { width: number; height: number } | null,
): Promise<{ context: BrowserContext; directory: string }> {
  const directory = mkdtempSync(join(tmpdir(), "lumenna-webkit-"));
  const context = await playwright.webkit.launchPersistentContext(directory, { baseURL, viewport });
  const page = context.pages()[0] ?? (await context.newPage());
  // A file of the origin that is not the app, so nothing holds the store while it goes.
  await page.goto("/manifest.webmanifest");
  await page.evaluate(async () => {
    const root = await navigator.storage.getDirectory();
    for await (const name of (root as unknown as { keys(): AsyncIterable<string> }).keys()) {
      await root.removeEntry(name, { recursive: true });
    }
    for (const database of await indexedDB.databases()) {
      if (database.name) indexedDB.deleteDatabase(database.name);
    }
    localStorage.clear();
    sessionStorage.clear();
  });
  await page.close();
  return { context, directory };
}

export const test = base.extend<Fixtures>({
  context: async ({ browserName, context, playwright, baseURL, viewport }, use) => {
    if (browserName !== "webkit") return use(context);
    const { context: persistent, directory } = await emptyWebKit(playwright, baseURL, viewport);
    await use(persistent);
    await persistent.close();
    rmSync(directory, { recursive: true, force: true });
  },
  freshContext: async ({ browserName, browser, playwright, baseURL, viewport }, use) => {
    const opened: { close(): Promise<void> }[] = [];
    await use(async () => {
      if (browserName !== "webkit") return opened[opened.push(await (browser as Browser).newContext()) - 1] as BrowserContext;
      // Two at once share one storage in WebKit: the second is only another tab of the first.
      const { context } = await emptyWebKit(playwright, baseURL, viewport);
      opened.push(context);
      return context;
    });
    for (const context of opened) await context.close();
  },
});
