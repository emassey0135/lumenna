// Devices and sync (§7, §16.12). The first test needs nothing outside the machine; the second
// pairs two browsers — two contexts, so two stores — over Iroh's public relays, so it needs the
// internet and runs only when asked: LUMENNA_NETWORK=1 npm test.

import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

async function open(page: Page) {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Lumenna", level: 1 })).toBeVisible({ timeout: 30_000 });
}

test("the devices dialog says this browser is not paired, and refuses a code that is not one", async ({ page }) => {
  await open(page);
  await page.getByRole("button", { name: "Devices…" }).click();
  const dialog = page.getByRole("dialog", { name: "Devices" });
  await expect(dialog.getByText(/^Not paired with any other device yet/)).toBeVisible();
  await expect(dialog.getByRole("button", { name: "Sync Now" })).toBeDisabled();
  const results = await new AxeBuilder({ page }).analyze();
  expect(results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target).join(", ")}`)).toEqual([]);
  await dialog.getByRole("button", { name: "Pair a Device…" }).click();
  const pairing = page.getByRole("dialog", { name: "Pair a Device" });
  await expect(pairing.getByRole("textbox", { name: "Name for this browser" })).toBeFocused();
  await pairing.getByRole("textbox", { name: "Code from the other device" }).fill("not-a-code");
  await pairing.getByRole("button", { name: "Pair With This Code" }).click();
  await expect(pairing.getByRole("alert")).toContainText("is not a pairing code");
  await pairing.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByRole("dialog", { name: "Devices" })).toBeVisible();
});

test("two browsers pair by code over a relay, and a task added in one arrives in the other", async ({ browser }) => {
  test.skip(!process.env.LUMENNA_NETWORK, "needs the internet: LUMENNA_NETWORK=1");
  test.setTimeout(240_000);
  const a = await (await browser.newContext()).newPage();
  const b = await (await browser.newContext()).newPage();
  await open(a);
  await open(b);

  const start = async (page: Page, name: string) => {
    await page.getByRole("button", { name: "Devices…" }).click();
    await page.getByRole("button", { name: "Pair a Device…" }).click();
    await page.getByRole("textbox", { name: "Name for this browser" }).fill(name);
  };

  await start(a, "Browser A");
  await a.getByRole("button", { name: "Wait for the Other Device" }).click();
  const code = a.getByRole("textbox", { name: "This browser's code" });
  await expect(code).toBeVisible({ timeout: 60_000 });
  await expect(code).toBeFocused();

  await start(b, "Browser B");
  await b.getByRole("textbox", { name: "Code from the other device" }).fill(await code.inputValue());
  await b.getByRole("button", { name: "Pair With This Code" }).click();

  // Both show the same three words, and only a yes on both pairs them.
  const words = (page: Page) => page.getByRole("alertdialog", { name: "Do These Words Match?" });
  await expect(words(a)).toBeVisible({ timeout: 120_000 });
  await expect(words(b)).toBeVisible({ timeout: 120_000 });
  const said = async (page: Page) => (await words(page).locator("p").textContent()) ?? "";
  expect(await said(a)).toEqual(await said(b));
  await words(a).getByRole("button", { name: "Yes, They Match" }).click();
  await words(b).getByRole("button", { name: "Yes, They Match" }).click();

  await expect(a.getByRole("option", { name: /^Browser B/ })).toBeVisible({ timeout: 120_000 });
  await expect(b.getByRole("option", { name: /^Browser A/ })).toBeVisible({ timeout: 120_000 });
  await a.getByRole("button", { name: "Close" }).click();
  await b.getByRole("button", { name: "Close" }).click();

  // Syncing runs while each page is open: a task added in one arrives in the other.
  await b.getByRole("button", { name: "New Task" }).click();
  await b.getByRole("combobox", { name: "Task" }).fill("Sent from the other browser");
  await b.getByRole("button", { name: "Add", exact: true }).click();
  await expect(a.getByRole("main").getByRole("row", { name: /^Sent from the other browser/ })).toBeVisible({
    timeout: 120_000,
  });
});

test("a browser and lum pair by code, and lum's sync brings its task to the browser", async ({ page }) => {
  const lum = process.env.LUMENNA_LUM;
  test.skip(!lum, "needs the internet and lum: LUMENNA_LUM=<path to lum>");
  test.setTimeout(240_000);
  const { spawn, execFileSync } = await import("node:child_process");
  const { mkdtempSync } = await import("node:fs");
  const { tmpdir } = await import("node:os");
  const { join } = await import("node:path");
  const home = mkdtempSync(join(tmpdir(), "lumenna-web-"));
  const env = { ...process.env, LUMENNA_PROFILE: join(home, "profile"), LUMENNA_BACKUP_DIR: join(home, "backups") };

  await open(page);
  await page.getByRole("button", { name: "Devices…" }).click();
  await page.getByRole("button", { name: "Pair a Device…" }).click();
  await page.getByRole("button", { name: "Wait for the Other Device" }).click();
  const code = page.getByRole("textbox", { name: "This browser's code" });
  await expect(code).toBeVisible({ timeout: 60_000 });

  const pairing = spawn(lum!, ["pair", await code.inputValue()], { env });
  let heard = "";
  pairing.stderr.on("data", (chunk: Buffer) => (heard += chunk.toString()));
  const exited = new Promise<number | null>((resolve) => pairing.on("exit", resolve));

  const words = page.getByRole("alertdialog", { name: "Do These Words Match?" });
  await expect(words).toBeVisible({ timeout: 120_000 });
  await expect.poll(() => heard, { timeout: 60_000 }).toContain("Type yes or no");
  const theirs = /The words are: (.*)\./.exec(heard)?.[1];
  expect(await words.locator("p").textContent()).toContain(theirs);
  pairing.stdin.write("yes\n");
  await words.getByRole("button", { name: "Yes, They Match" }).click();
  expect(await exited).toBe(0);
  const devices = page.getByRole("dialog", { name: "Devices" });
  await expect(devices.getByRole("option").nth(1)).toBeVisible({ timeout: 60_000 });
  await expect(devices.getByText(/^Sync is running, with 1 other device/)).toBeVisible({ timeout: 60_000 });
  await page.getByRole("button", { name: "Close" }).click();

  // lum dials the browser, which answers while its page is open. A browser's endpoint is
  // reached through the relay it registered with, which can take a moment after it opens: a
  // round that misses it is tried again, as a daemon's loop would.
  execFileSync(lum!, ["task", "add", "Sent from lum"], { env });
  let report = "";
  for (let attempt = 0; attempt < 4 && !report.includes("Synced with 1 of 1"); attempt++) {
    report = execFileSync(lum!, ["sync"], { env, timeout: 120_000 }).toString();
  }
  expect(report).toContain("Synced with 1 of 1");
  await expect(page.getByRole("main").getByRole("row", { name: /^Sent from lum/ })).toBeVisible({ timeout: 60_000 });
});
