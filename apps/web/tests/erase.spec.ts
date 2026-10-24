// Real erasure: a trashed task's Erase Permanently, a menu away and never a key's; Settings'
// Erase Everything Deleted; and the backups a folder holds, which hold what was erased, replaced
// afterwards — read back through the accessibility tree, as the other tests are.

import { expect, test } from "./fixtures";
import type { Page } from "./fixtures";
import AxeBuilder from "@axe-core/playwright";

async function open(page: Page) {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Lumenna", level: 1 })).toBeVisible({ timeout: 30_000 });
}

/** Adds a task and moves it to the trash with Delete, as a person would. */
async function trashed(page: Page, title: string) {
  await page.getByRole("button", { name: "New task" }).click();
  await page.getByRole("combobox", { name: "Task" }).fill(title);
  await page.getByRole("button", { name: "Add", exact: true }).click();
  await expect(page.getByRole("dialog")).toBeHidden();
  const row = page.getByRole("main").getByRole("row", { name: new RegExp(`^${title}`) });
  await row.focus();
  await page.keyboard.press("Delete");
  await expect(row).toHaveCount(0);
}

async function place(page: Page, name: RegExp) {
  await page.getByRole("navigation", { name: "Places" }).getByRole("row", { name }).click();
}

async function axe(page: Page) {
  const results = await new AxeBuilder({ page }).analyze();
  expect(results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target).join(", ")}`)).toEqual([]);
}

/** Opens Settings at its Backups page. */
async function backups(page: Page) {
  await page.getByRole("button", { name: "Settings" }).click();
  await page.getByRole("tab", { name: "Backups" }).click();
}

/** Erases everything deleted from Settings, answering the app's question. */
async function eraseEverything(page: Page) {
  await page.getByRole("button", { name: "Erase everything deleted", exact: true }).click();
  const question = page.getByRole("alertdialog", { name: /^Erase everything deleted permanently\?$/i });
  await question.getByRole("button", { name: /^Erase everything deleted$/i }).click();
  await expect(question).toBeHidden();
}

test("a trashed task offers Erase permanently in its menu after Delete from trash, and Delete never runs it", async ({ page }) => {
  await open(page);
  await trashed(page, "Secret plan");
  await place(page, /^Trash/);
  const row = page.getByRole("main").getByRole("row", { name: /^Secret plan/ });
  await row.focus();
  await page.keyboard.press("Shift+F10");
  await expect(page.getByRole("menuitem")).toHaveText(["Restore", "Delete from trash", "Erase permanently"]);
  await axe(page);
  await page.keyboard.press("Escape");
  await expect(row).toBeFocused();

  // Delete asks Delete from Trash's question, not erasing's.
  await page.keyboard.press("Delete");
  const deleting = page.getByRole("alertdialog", { name: "Delete Secret plan from the trash?" });
  await expect(deleting).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(deleting).toBeHidden();
  await expect(row).toBeFocused();

  await page.keyboard.press("Shift+F10");
  await page.getByRole("menuitem", { name: "Erase permanently" }).click();
  const erasing = page.getByRole("alertdialog", { name: "Erase Secret plan permanently?" });
  await expect(erasing).toHaveAccessibleDescription(/^Permanently erase — rebuilds your database and re-syncs all devices\./);
  await expect(erasing.getByRole("button", { name: "Cancel" })).toBeFocused();
  await axe(page);
  await erasing.getByRole("button", { name: /^Erase permanently$/i }).click();
  await expect(erasing).toBeHidden();
  await expect(row).toHaveCount(0);
});

test("Settings offers Erase everything deleted beside the backups, asking first, and the trash is emptied", async ({ page }) => {
  await open(page);
  await trashed(page, "Old receipt");
  await backups(page);
  const erase = page.getByRole("button", { name: "Erase everything deleted", exact: true });
  await expect(erase).toBeVisible();
  await axe(page);
  await erase.click();
  const question = page.getByRole("alertdialog", { name: /^Erase everything deleted permanently\?$/i });
  await expect(question).toHaveAccessibleDescription(/^Permanently erase — rebuilds your database and re-syncs all devices\./);
  await expect(question.getByRole("button", { name: "Cancel" })).toBeFocused();
  await axe(page);
  await question.getByRole("button", { name: "Cancel" }).click();
  await expect(question).toBeHidden();

  await eraseEverything(page);
  await page.getByRole("button", { name: "Close", exact: true }).click();
  await place(page, /^Trash/);
  await expect(page.getByRole("main").getByRole("heading", { level: 2 })).toHaveText("Trash");
  await expect(page.getByRole("main").getByRole("row", { name: /^Old receipt/ })).toHaveCount(0);
});

test("after an erasure every backup in the chosen folder is replaced by one of the history begun again", async ({ page, browserName }) => {
  // The File System Access API's folder is Chromium's; elsewhere the page offers only downloads.
  test.skip(browserName !== "chromium", "only Chromium can be given a folder");
  // The folder picker stands in for the system's, as in manage.spec.ts: a folder in this origin's
  // private storage, holding two old backups and a file of the person's own.
  await page.addInitScript(() => {
    (window as unknown as { showDirectoryPicker: () => Promise<FileSystemDirectoryHandle> }).showDirectoryPicker = async () => {
      const root = await navigator.storage.getDirectory();
      const folder = await root.getDirectoryHandle("picked", { create: true });
      for (const name of ["lumenna-20260110T090000.000Z.lumbak", "lumenna-20260111T090000.000Z.lumbak", "notes.txt"]) {
        await (await (await folder.getFileHandle(name, { create: true })).createWritable()).close();
      }
      return folder;
    };
  });
  await open(page);
  await trashed(page, "Bank details");
  await backups(page);
  await page.getByRole("button", { name: "Choose a folder for backups" }).click();
  await expect(page.getByText("Backing up to the folder picked.")).toBeVisible();
  const names = () =>
    page.evaluate(async () => {
      const folder = await (await navigator.storage.getDirectory()).getDirectoryHandle("picked");
      const found: string[] = [];
      for await (const name of (folder as unknown as { keys(): AsyncIterable<string> }).keys()) found.push(name);
      return found.sort();
    });
  await expect.poll(names).toHaveLength(4);
  const before = (await names()).filter((name) => name.endsWith(".lumbak"));

  await eraseEverything(page);
  // Each of the three held the task erased: gone, one taken afresh, and the person's file kept.
  await expect.poll(names, { timeout: 20_000 }).toHaveLength(2);
  const after = await names();
  expect(after).toContain("notes.txt");
  const fresh = after.filter((name) => name.endsWith(".lumbak"));
  expect(fresh).toHaveLength(1);
  expect(before).not.toContain(fresh[0]);
});
