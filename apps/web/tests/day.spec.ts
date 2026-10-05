// The day, blocks, and asking before what cannot be undone — read back through the browser's
// accessibility tree, as the other tests are.

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

async function place(page: Page, name: RegExp) {
  await page.getByRole("navigation", { name: "Places" }).getByRole("row", { name }).click();
}

async function addBlock(page: Page, title: string, at: string, repeat = "") {
  await page.getByRole("button", { name: "Add Block…" }).click();
  const form = page.getByRole("dialog", { name: "New Block" });
  await expect(form.getByRole("textbox", { name: "Title", exact: true })).toBeFocused();
  await form.getByRole("textbox", { name: "Title", exact: true }).fill(title);
  await form.getByRole("textbox", { name: "Starts at" }).fill(at);
  await form.getByRole("textbox", { name: "Minutes", exact: true }).fill("60");
  if (repeat) await form.getByRole("textbox", { name: "Repeats", exact: true }).fill(repeat);
  await form.getByRole("button", { name: "Add", exact: true }).click();
  await expect(form).toBeHidden();
}

async function axe(page: Page) {
  const results = await new AxeBuilder({ page }).analyze();
  expect(results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target).join(", ")}`)).toEqual([]);
}

test("erasing asks first, with Cancel focused, and Cancel keeps the task", async ({ page }) => {
  await open(page);
  await add(page, "Old idea");
  const tasks = page.getByRole("main");
  await tasks.getByRole("row", { name: /^Old idea/ }).focus();
  await page.keyboard.press("Delete");
  await place(page, /^Trash/);
  await tasks.getByRole("row", { name: /^Old idea/ }).focus();
  await page.keyboard.press("Delete");
  const question = page.getByRole("alertdialog", { name: "Erase Old idea?" });
  await expect(question).toBeVisible();
  await expect(question).toHaveAccessibleDescription(/cannot be undone/);
  await expect(question.getByRole("button", { name: "Cancel" })).toBeFocused();
  await axe(page);
  await page.keyboard.press("Escape");
  await expect(question).toBeHidden();
  await expect(tasks.getByRole("row", { name: /^Old idea/ })).toBeFocused();
  await page.keyboard.press("Delete");
  await question.getByRole("button", { name: "Erase" }).click();
  await expect(tasks.getByRole("row", { name: /^Old idea/ })).toHaveCount(0);
});

test("the day opens with its summary, and a block added there is listed", async ({ page }) => {
  await open(page);
  await place(page, /^Today/);
  const day = page.getByRole("treegrid", { name: "The day" });
  await expect(day.getByRole("row").first()).toHaveAccessibleName(/^Today/);
  await addBlock(page, "Deep work", "9am");
  await expect(day.getByRole("row", { name: /Deep work, 1 hour, work block/ })).toBeVisible();
  await axe(page);
});

test("a task assigned from a block is a sitting beneath it, and Space times it", async ({ page }) => {
  await open(page);
  await add(page, "Write report");
  await place(page, /^Today/);
  await addBlock(page, "Deep work", "11:00pm");
  const day = page.getByRole("treegrid", { name: "The day" });
  const block = day.getByRole("row", { name: /Deep work/ });
  await block.focus();
  await page.keyboard.press("Shift+F10");
  await page.getByRole("menuitem", { name: "Assign a Task…" }).click();
  const picker = page.getByRole("dialog", { name: "Assign to Deep work" });
  await picker.getByRole("combobox", { name: "Task" }).fill("Write");
  await page.getByRole("option", { name: /^Write report/ }).click();
  await picker.getByRole("button", { name: "OK" }).click();
  const minutes = page.getByRole("dialog", { name: /Meant to Take/ });
  await minutes.getByRole("textbox", { name: "Minutes", exact: true }).fill("30");
  await minutes.getByRole("button", { name: "OK" }).click();
  const sitting = day.getByRole("row", { name: /^Write report/ });
  await expect(sitting).toHaveAttribute("aria-level", "2");
  await expect(block).toHaveAccessibleName(/1 task assigned/);
  await sitting.focus();
  await page.keyboard.press("Space");
  await expect(sitting).toHaveAccessibleName(/in progress/);
  await expect(sitting).toBeFocused();
});

test("a sitting's minutes refuse what is not a number, at the field", async ({ page }) => {
  await open(page);
  await add(page, "Write report");
  await place(page, /^Today/);
  await addBlock(page, "Deep work", "11:00pm");
  const day = page.getByRole("treegrid", { name: "The day" });
  await day.getByRole("row", { name: /Deep work/ }).focus();
  await page.keyboard.press("Shift+F10");
  await page.getByRole("menuitem", { name: "Assign a Task…" }).click();
  await page.getByRole("combobox", { name: "Task" }).fill("Write");
  await page.getByRole("option", { name: /^Write report/ }).click();
  await page.getByRole("button", { name: "OK" }).click();
  const field = page.getByRole("textbox", { name: "Minutes", exact: true });
  await field.fill("soon");
  await page.getByRole("button", { name: "OK" }).click();
  await expect(field).toHaveAttribute("aria-invalid", "true");
  await expect(field).toHaveAccessibleDescription(/not a number of minutes/);
});

test("moving to the next day says it, and Today comes back", async ({ page }) => {
  await open(page);
  await place(page, /^Today/);
  await page.getByRole("button", { name: "Next Day" }).click();
  await expect(page.getByRole("main").getByRole("heading", { level: 2 })).toHaveText("Tomorrow");
  await expect(page.locator('[aria-live="polite"]')).toContainText("Tomorrow");
  await page.getByRole("button", { name: "Today", exact: true }).click();
  await expect(page.getByRole("main").getByRole("heading", { level: 2 })).toHaveText("Today");
});

test("a repeating block asks which occurrences a change is for", async ({ page }) => {
  await open(page);
  await place(page, /^Today/);
  await addBlock(page, "Standup", "11:30pm", "every day");
  const day = page.getByRole("treegrid", { name: "The day" });
  await day.getByRole("row", { name: /Standup/ }).focus();
  await page.keyboard.press("Enter");
  const which = page.getByRole("alertdialog", { name: "Change Standup" });
  await expect(which.getByRole("button", { name: "Cancel" })).toBeFocused();
  await which.getByRole("button", { name: "Today Only" }).click();
  const form = page.getByRole("dialog", { name: "Change Standup, This Day Only" });
  await expect(form.getByRole("textbox", { name: "Repeats", exact: true })).toHaveCount(0);
  await form.getByRole("textbox", { name: "Minutes", exact: true }).fill("15");
  await form.getByRole("button", { name: "Save" }).click();
  await expect(day.getByRole("row", { name: /Standup, 15 minutes.*changed for this day/ })).toBeVisible();
});

test("the blocks list every series, and Delete asks before deleting one", async ({ page }) => {
  await open(page);
  await place(page, /^Blocks/);
  await addBlock(page, "Gym", "7am", "every weekday");
  const blocks = page.getByRole("treegrid", { name: "Blocks" });
  const gym = blocks.getByRole("row", { name: /^Gym/ });
  await expect(gym).toBeVisible();
  await axe(page);
  await gym.focus();
  await page.keyboard.press("Delete");
  await page.getByRole("alertdialog", { name: "Delete Gym?" }).getByRole("button", { name: "Delete" }).click();
  await expect(gym).toHaveCount(0);
});

test("a task is put in a block from its own details", async ({ page }) => {
  await open(page);
  await place(page, /^Today/);
  await addBlock(page, "Deep work", "11:00pm");
  await add(page, "Write report");
  await place(page, /^Tasks/);
  await page.getByRole("main").getByRole("row", { name: /^Write report/ }).click();
  await page.getByRole("complementary", { name: "Task details" }).getByRole("button", { name: "Put in a Block…" }).click();
  const picker = page.getByRole("dialog", { name: "Put Write report in a Block" });
  await picker.getByRole("combobox", { name: "Block" }).fill("Deep");
  await page.getByRole("option", { name: /Deep work/ }).click();
  await picker.getByRole("button", { name: "OK" }).click();
  await page.getByRole("dialog", { name: /Meant to Take/ }).getByRole("button", { name: "OK" }).click();
  await place(page, /^Today/);
  await expect(page.getByRole("treegrid", { name: "The day" }).getByRole("row", { name: /^Write report/ })).toBeVisible();
});
