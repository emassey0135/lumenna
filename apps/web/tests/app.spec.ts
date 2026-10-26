// The web client as a screen reader user drives it, read back through the browser's
// accessibility tree — Playwright's role queries — as the Windows UI tests read UI Automation.

import { expect, test } from "./fixtures";
import type { Page } from "./fixtures";
import AxeBuilder from "@axe-core/playwright";

async function open(page: Page) {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Lumenna", level: 1 })).toBeVisible({ timeout: 30_000 });
}

async function add(page: Page, line: string) {
  await page.getByRole("button", { name: "New task" }).click();
  const field = page.getByRole("combobox", { name: "Task" });
  await expect(field).toBeFocused();
  await field.fill(line);
  await page.getByRole("button", { name: "Add" }).click();
  await expect(page.getByRole("dialog")).toBeHidden();
}

test("the store opens in the browser and the places are a tree", async ({ page }) => {
  await open(page);
  const places = page.getByRole("navigation", { name: "Places" });
  await expect(places.getByRole("row", { name: /^Tasks/ })).toBeVisible();
  await expect(places.getByRole("row", { name: /^Inbox/ })).toBeVisible();
});

test("a second tab says the store is open in the first, and opens it once the first is closed", async ({ context }) => {
  const first = await context.newPage();
  await open(first);
  const second = await context.newPage();
  await second.goto("/");
  await expect(second.getByRole("alert")).toHaveText("Lumenna is open in another tab. Close it there to use it here.", {
    timeout: 30_000,
  });
  await first.close();
  await expect(second.getByRole("navigation", { name: "Places" }).getByRole("row", { name: /^Tasks/ })).toBeVisible({ timeout: 30_000 });
  await expect(second.getByRole("alert")).toHaveCount(0);
});

test("a launch shortcut in the address does what it says once, and leaves the address", async ({ page }) => {
  await page.goto("/?launch=new-task");
  const field = page.getByRole("combobox", { name: "Task" });
  await expect(field).toBeFocused({ timeout: 30_000 });
  await expect(page).toHaveURL((url) => !url.searchParams.has("launch"));
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toBeHidden();

  await page.goto("/?launch=today");
  await expect(page.getByRole("main").getByRole("heading", { level: 2 })).toHaveText("Today", { timeout: 30_000 });
  await expect(page).toHaveURL((url) => !url.searchParams.has("launch"));
  // Reloaded, the address no longer asks for anything: quick add stays closed.
  await page.reload();
  await expect(page.getByRole("heading", { name: "Lumenna", level: 1 })).toBeVisible({ timeout: 30_000 });
  await expect(page.getByRole("dialog")).toBeHidden();
});

test("quick add reads back what it will add, and adds it", async ({ page }) => {
  await open(page);
  await page.getByRole("button", { name: "New task" }).click();
  await page.getByRole("combobox", { name: "Task" }).fill("Call Sam tomorrow p1");
  await expect(page.getByRole("status").or(page.locator("output"))).toContainText("Call Sam");
  await expect(page.locator("output")).toContainText("priority 1");
  await page.getByRole("button", { name: "Add" }).click();
  await expect(page.getByRole("main").getByRole("row", { name: /^Call Sam/ })).toBeVisible();
});

test("what the app says is shown on the status line, for someone who reads it", async ({ page }) => {
  await open(page);
  await add(page, "Buy milk");
  const tasks = page.getByRole("main");
  await tasks.getByRole("row", { name: /^Buy milk/ }).focus();
  await page.keyboard.press("Space");
  await expect(page.getByRole("contentinfo")).toContainText("Buy milk");
  const audit = await new AxeBuilder({ page }).analyze();
  expect(audit.violations).toEqual([]);
});

test("history lists a task done today, said as when it was done, beside the trash", async ({ page }) => {
  await open(page);
  await add(page, "Buy milk");
  await page.getByRole("main").getByRole("row", { name: /^Buy milk/ }).focus();
  await page.keyboard.press("Space");
  await page.getByRole("navigation", { name: "Places" }).getByRole("row", { name: /^History, 1 task done/ }).click();
  await expect(page.getByRole("main", { name: "History" })).toBeVisible();
  await expect(page.getByRole("main").getByRole("row", { name: /^Buy milk, completed today/ })).toBeVisible();
  const audit = await new AxeBuilder({ page }).analyze();
  expect(audit.violations).toEqual([]);
});

test("a task due at a time says it in the browser's clock, then its priority", async ({ page }) => {
  await open(page);
  await add(page, "Call the bank tomorrow at 3pm p1");
  // The browser's locale words the time; Playwright's default is en-US, and Intl may put a
  // narrow no-break space before PM.
  await expect(page.getByRole("main").getByRole("row", { name: /^Call the bank, due tomorrow at 3:00\sPM, priority 1/ }))
    .toBeVisible();
});

test("space checks a task off and focus moves to the one that took its place", async ({ page }) => {
  await open(page);
  await add(page, "Buy milk");
  await add(page, "Call Sam");
  const tasks = page.getByRole("main");
  await tasks.getByRole("row", { name: /^Buy milk/ }).focus();
  await page.keyboard.press("Space");
  await expect(tasks.getByRole("row", { name: /^Buy milk/ })).toHaveCount(0);
  await expect(tasks.getByRole("row", { name: /^Call Sam/ })).toBeFocused();
});

test("completion offers what fits at the cursor, and puts it in that span", async ({ page }) => {
  await open(page);
  await page.getByRole("button", { name: "New task" }).click();
  const field = page.getByRole("combobox", { name: "Task" });
  await field.fill("Call #Inb tomorrow");
  // The cursor after "#Inb", mid-line.
  await field.evaluate((input: HTMLInputElement) => input.setSelectionRange(9, 9));
  await field.press("ArrowRight");
  await field.press("ArrowLeft");
  await field.press("ArrowDown");
  const option = page.getByRole("option", { name: "Inbox, project" });
  await expect(option).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(field).toHaveValue("Call #Inbox tomorrow");
});

test("the details save only what changed", async ({ page }) => {
  await open(page);
  await add(page, "Write report");
  await page.getByRole("main").getByRole("row", { name: /^Write report/ }).click();
  const details = page.getByRole("complementary", { name: "Task details" });
  const title = details.getByRole("textbox", { name: "Title" });
  await details.getByRole("button", { name: /Priority/ }).click();
  await expect(page.getByRole("option")).toHaveText(["Priority 1, highest", "Priority 2", "Priority 3", "Priority 4, none"]);
  await page.keyboard.press("Escape");
  await title.fill("Write the report");
  await details.getByRole("button", { name: "Save" }).click();
  await expect(page.getByRole("main").getByRole("row", { name: /^Write the report/ })).toBeVisible();
});

test("nothing on the page breaks axe's accessibility rules", async ({ page }) => {
  await open(page);
  await add(page, "Buy milk");
  await page.getByRole("main").getByRole("row", { name: /^Buy milk/ }).click();
  const results = await new AxeBuilder({ page }).analyze();
  expect(results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target).join(", ")}`)).toEqual([]);
});

test("a row's check box marks it done at a click, and is no part of what a screen reader hears", async ({ page }) => {
  await open(page);
  await add(page, "Buy milk");
  await add(page, "Call Sam");
  const tasks = page.getByRole("main");
  const row = tasks.getByRole("row", { name: /^Buy milk/ });
  // Hidden from assistive technology: the row's name is its line alone, and no control is added.
  await expect(row).toHaveAccessibleName(/^Buy milk/);
  await expect(row.getByRole("checkbox")).toHaveCount(0);
  await expect(row.getByRole("button")).toHaveCount(0);
  await row.locator(".done-box").click();
  await expect(tasks.getByRole("row", { name: /^Buy milk/ })).toHaveCount(0);
  // As after Space: the task now in its place is the one in hand, not the one marked done.
  await expect(tasks.getByRole("row", { name: /^Call Sam/ })).toBeFocused();
});

test("the row in hand is not said to be selected again each time the screen is drawn", async ({ page }) => {
  await open(page);
  await page.getByRole("navigation", { name: "Places" }).getByRole("row", { name: /^Blocks/ }).click();
  await page.getByRole("button", { name: "New block" }).click();
  const form = page.getByRole("dialog", { name: "New Block" });
  await form.getByRole("textbox", { name: "Name", exact: true }).fill("Deep work");
  await form.getByRole("textbox", { name: "Starts at" }).fill("9am");
  await form.getByRole("button", { name: "Add", exact: true }).click();
  await expect(form).toBeHidden();
  const row = page.getByRole("treegrid", { name: "Blocks" }).getByRole("row", { name: /^Deep work/ });
  await row.focus();
  // Each change, and each look at what the row offers, draws the screen again.
  for (let i = 0; i < 3; i++) {
    await page.keyboard.press("Shift+F10");
    await page.keyboard.press("Escape");
  }
  await page.waitForTimeout(1000);
  const said = (await page.locator("[data-live-announcer]").textContent()) ?? "";
  expect(said.split("selected").length - 1).toBeLessThanOrEqual(1);
});

test("a sidebar heading folds with Left and opens with Right, through the app", async ({ page }) => {
  await open(page);
  const places = page.getByRole("navigation", { name: "Places" });
  const projects = places.getByRole("row", { name: /^Projects/ });
  await projects.focus();
  await page.keyboard.press("ArrowLeft");
  await expect(places.getByRole("row", { name: /^Inbox/ })).toHaveCount(0);
  await expect(projects).toHaveAttribute("aria-expanded", "false");
  await expect(page.locator('[aria-live="polite"]')).toContainText("Collapsed");
  await page.keyboard.press("ArrowRight");
  await expect(places.getByRole("row", { name: /^Inbox/ })).toBeVisible();
});
