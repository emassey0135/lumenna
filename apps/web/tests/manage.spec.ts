// What the desktop apps offer beyond tasks and the day: projects, labels and saved filters,
// waiting for other tasks, settings, backups, every block setting, pausing timers, and undo
// from the keyboard — read back through the accessibility tree, as the other tests are.

import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

async function open(page: Page) {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Lumenna", level: 1 })).toBeVisible({ timeout: 30_000 });
}

async function add(page: Page, line: string) {
  await page.getByRole("button", { name: "New Task" }).click();
  await page.getByRole("combobox", { name: "Task" }).fill(line);
  await page.getByRole("button", { name: "Add", exact: true }).click();
  await expect(page.getByRole("dialog")).toBeHidden();
}

const places = (page: Page) => page.getByRole("navigation", { name: "Places" });
const heading = (page: Page) => page.getByRole("main").getByRole("heading", { level: 2 });

/** Opens a sidebar row's menu from the keyboard and chooses `item`. */
async function placeMenu(page: Page, row: RegExp, item: string) {
  await places(page).getByRole("row", { name: row }).focus();
  await page.keyboard.press("Shift+F10");
  await page.getByRole("menuitem", { name: item }).click();
}

/** Answers an ask dialog called `title`. */
async function answer(page: Page, title: string | RegExp, text: string) {
  const dialog = page.getByRole("dialog", { name: title });
  await dialog.getByRole("textbox").first().fill(text);
  await dialog.getByRole("button", { name: "OK" }).click();
  await expect(dialog).toBeHidden();
}

async function axe(page: Page) {
  const results = await new AxeBuilder({ page }).analyze();
  expect(results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target).join(", ")}`)).toEqual([]);
}

test("a project is made, renamed, and deleted keeping its tasks, from the sidebar", async ({ page }) => {
  await open(page);
  await placeMenu(page, /^Projects/, "New Project");
  await answer(page, "New Project", "Garden");
  await expect(heading(page)).toHaveText("Garden");
  await add(page, "Plant beans");
  await placeMenu(page, /^Garden/, "Rename");
  await answer(page, "Rename Garden", "Allotment");
  await expect(heading(page)).toHaveText("Allotment");
  await places(page).getByRole("row", { name: /^Allotment/ }).focus();
  await page.keyboard.press("Delete");
  const question = page.getByRole("alertdialog", { name: "Delete Allotment?" });
  await expect(question.getByRole("button", { name: "Cancel" })).toBeFocused();
  await question.getByRole("button", { name: "Delete and Keep Its Tasks" }).click();
  await expect(places(page).getByRole("row", { name: /^Allotment/ })).toHaveCount(0);
  await expect(places(page).getByRole("row", { name: /^Inbox, 1 open task/ })).toBeVisible();
});

test("a label and a saved filter are made and changed from the sidebar", async ({ page }) => {
  await open(page);
  await placeMenu(page, /^Labels/, "New Label");
  await answer(page, "New Label", "calls");
  await expect(heading(page)).toHaveText("calls");
  await placeMenu(page, /^calls/, "Colour");
  await answer(page, "Colour of calls", "teal");
  await placeMenu(page, /^Saved Filters/, "New Saved Filter");
  await answer(page, "New Saved Filter", "Urgent");
  await answer(page, "Query for Urgent", "p1");
  await expect(heading(page)).toHaveText("Urgent");
  await placeMenu(page, /^Urgent/, "Change Query");
  await answer(page, "Query of Urgent", "p1 | today");
  await expect(page.getByRole("main").getByRole("textbox", { name: "Filter" })).toHaveValue("p1 | today");
  await axe(page);
});

test("a task waits for another, and stops", async ({ page }) => {
  await open(page);
  await add(page, "Buy paint");
  await add(page, "Paint the fence");
  await page.getByRole("main").getByRole("row", { name: /^Paint the fence/ }).click();
  const details = page.getByRole("complementary", { name: "Task details" });
  await details.getByRole("button", { name: "Wait For" }).click();
  const picker = page.getByRole("dialog", { name: "What does Paint the fence wait for?" });
  await picker.getByRole("combobox", { name: "What does Paint the fence wait for?" }).fill("Buy");
  await page.getByRole("option", { name: /^Buy paint/ }).click();
  await picker.getByRole("button", { name: "OK" }).click();
  const stop = details.getByRole("button", { name: "Stop Waiting for Buy paint" });
  await expect(stop).toBeVisible();
  await stop.click();
  await expect(details.getByText("Nothing.")).toBeVisible();
});

test("a planning setting applies when its field is left, and says so", async ({ page }) => {
  await open(page);
  await page.getByRole("button", { name: "Settings…" }).click();
  const settings = page.getByRole("dialog", { name: "Settings" });
  await expect(settings.getByRole("tab", { name: "Planning" })).toHaveAttribute("aria-selected", "true");
  await axe(page);
  await settings.getByRole("textbox", { name: "Day ends" }).fill("21:30");
  await settings.getByRole("textbox", { name: "Day ends" }).press("Tab");
  await expect(page.locator('[aria-live="polite"]')).toContainText("21:30");
  await settings.getByRole("button", { name: "Close" }).click();
  await page.getByRole("button", { name: "Settings…" }).click();
  await expect(page.getByRole("textbox", { name: "Day ends" })).toHaveValue("21:30");
});

test("a backup downloads, and restores into another browser", async ({ browser }) => {
  const here = await (await browser.newContext()).newPage();
  await open(here);
  await add(here, "Remember this");
  await here.getByRole("button", { name: "Settings…" }).click();
  await here.getByRole("tab", { name: "Backups" }).click();
  const downloading = here.waitForEvent("download");
  await here.getByRole("button", { name: "Download a Backup" }).click();
  const file = await downloading;
  expect(file.suggestedFilename()).toMatch(/^lumenna-.*\.lumbak$/);
  const path = await file.path();

  const there = await (await browser.newContext()).newPage();
  await open(there);
  await there.getByRole("button", { name: "Settings…" }).click();
  await there.getByRole("tab", { name: "Backups" }).click();
  const choosing = there.waitForEvent("filechooser");
  await there.getByRole("button", { name: "Restore from a Backup…" }).click();
  await (await choosing).setFiles(path);
  // A restore merges every document in the worker, which takes a while beside other tests.
  await expect(there.locator('[aria-live="polite"]')).toContainText("Restored", { timeout: 20_000 });
  await there.getByRole("button", { name: "Close" }).click();
  await expect(there.getByRole("main").getByRole("row", { name: /^Remember this/ })).toBeVisible();
});

test("a block's flags follow its kind, and its settings are kept", async ({ page }) => {
  await open(page);
  await places(page).getByRole("row", { name: /^Blocks/ }).click();
  await page.getByRole("button", { name: "Add Block…" }).click();
  const form = page.getByRole("dialog", { name: "New Block" });
  await form.getByRole("textbox", { name: "Title", exact: true }).fill("Commute");
  await form.getByRole("button", { name: /Kind/ }).click();
  await page.getByRole("option", { name: "Break" }).click();
  const takes = form.getByRole("checkbox", { name: "Takes tasks" });
  await expect(takes).not.toBeChecked();
  // Someone who works on the train.
  await takes.focus();
  await page.keyboard.press("Space");
  await expect(takes).toBeChecked();
  await form.getByRole("textbox", { name: "Offers tasks matching this filter" }).fill("#Inbox");
  await form.getByRole("textbox", { name: "Colour" }).fill("teal");
  await axe(page);
  await form.getByRole("button", { name: "Add", exact: true }).click();
  await expect(form).toBeHidden();
  await page.getByRole("treegrid", { name: "Blocks" }).getByRole("row", { name: /^Commute/ }).focus();
  await page.keyboard.press("Enter");
  const change = page.getByRole("dialog", { name: /^Change Commute/ });
  await expect(change.getByRole("checkbox", { name: "Takes tasks" })).toBeChecked();
  await expect(change.getByRole("textbox", { name: "Offers tasks matching this filter" })).toHaveValue("#Inbox");
  await expect(change.getByRole("textbox", { name: "Colour" })).toHaveValue("teal");
});

test("space pauses a running timer, and the menu offers resume and stop", async ({ page }) => {
  await open(page);
  await add(page, "Write report");
  await places(page).getByRole("row", { name: /^Today/ }).click();
  await page.getByRole("button", { name: "Add Block…" }).click();
  const form = page.getByRole("dialog", { name: "New Block" });
  await form.getByRole("textbox", { name: "Title", exact: true }).fill("Deep work");
  await form.getByRole("textbox", { name: "Starts at" }).fill("11:30pm");
  await form.getByRole("textbox", { name: "Minutes", exact: true }).fill("25");
  await form.getByRole("button", { name: "Add", exact: true }).click();
  const day = page.getByRole("treegrid", { name: "The day" });
  await day.getByRole("row", { name: /Deep work/ }).focus();
  await page.keyboard.press("Shift+F10");
  await page.getByRole("menuitem", { name: "Assign a Task" }).click();
  await page.getByRole("combobox", { name: "Assign a task to Deep work" }).fill("Write");
  await page.getByRole("option", { name: /^Write report/ }).click();
  await page.getByRole("button", { name: "OK" }).click();
  await page.getByRole("dialog", { name: "Planned length" }).getByRole("button", { name: "OK" }).click();
  const sitting = day.getByRole("row", { name: /^Write report/ });
  await sitting.focus();
  await page.keyboard.press("Space");
  await expect(sitting).toHaveAccessibleName(/in progress/);
  await page.keyboard.press("Space");
  await expect(sitting).toHaveAccessibleName(/paused/);
  await page.keyboard.press("Shift+F10");
  await expect(page.getByRole("menuitem", { name: "Resume Timer" })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "Stop Timer" })).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "Pause Timer" })).toHaveCount(0);
});

test("ctrl+z undoes from the list, and ctrl+y redoes", async ({ page }) => {
  await open(page);
  await add(page, "Buy milk");
  const row = page.getByRole("main").getByRole("row", { name: /^Buy milk/ });
  await row.focus();
  await page.keyboard.press("Control+z");
  await expect(row).toHaveCount(0);
  await page.getByRole("main").getByRole("treegrid").focus();
  await page.keyboard.press("Control+y");
  await expect(row).toBeVisible();
});

test("a weight that is not a number is refused at the field, not taken as inherit", async ({ page }) => {
  await open(page);
  await placeMenu(page, /^Projects/, "New Project");
  await answer(page, "New Project", "Garden");
  await placeMenu(page, /^Garden/, "Weight");
  const dialog = page.getByRole("dialog", { name: "Weight of Garden" });
  const field = dialog.getByRole("textbox", { name: "Weight" });
  await field.fill("1,5");
  await dialog.getByRole("button", { name: "OK" }).click();
  await expect(field).toHaveAttribute("aria-invalid", "true");
  await expect(field).toHaveAccessibleDescription(/is not a weight/);
  await field.fill("1.5");
  await dialog.getByRole("button", { name: "OK" }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator('[aria-live="polite"]')).toContainText("Garden");
});
