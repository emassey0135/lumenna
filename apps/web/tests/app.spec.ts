// The web client as a screen reader user drives it, read back through the browser's
// accessibility tree — Playwright's role queries — as the Windows UI tests read UI Automation.

import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";
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

test("quick add reads back what it will add, and adds it", async ({ page }) => {
  await open(page);
  await page.getByRole("button", { name: "New task" }).click();
  await page.getByRole("combobox", { name: "Task" }).fill("Call Sam tomorrow p1");
  await expect(page.getByRole("status").or(page.locator("output"))).toContainText("Call Sam");
  await expect(page.locator("output")).toContainText("priority 1");
  await page.getByRole("button", { name: "Add" }).click();
  await expect(page.getByRole("main").getByRole("row", { name: /^Call Sam/ })).toBeVisible();
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
