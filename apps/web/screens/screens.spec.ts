// Screenshots of each of the web client's screens, for a visual review: light and dark, desktop
// and phone widths. Not a test of behaviour; every step only gets to a screen.

import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

const OUT = process.env.SHOTS ?? "screens/out";
const MORNING = new Date(2030, 0, 15, 10, 40);

async function at(page: Page, time: Date) {
  const target = time.getTime();
  await page.route(/\/src\/worker\.ts/, async (route) => {
    const response = await route.fetch();
    const shim =
      `{const Real=Date;const offset=${target}-Real.now();` +
      `class Shifted extends Real{constructor(...a){if(a.length)super(...a);else super(Real.now()+offset)}static now(){return Real.now()+offset}}` +
      `globalThis.Date=Shifted;}\n`;
    await route.fulfill({ response, body: shim + (await response.text()) });
  });
  await page.clock.install({ time });
}

async function open(page: Page) {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Lumenna", level: 1 })).toBeVisible({ timeout: 30_000 });
}

async function add(page: Page, line: string) {
  await page.getByRole("button", { name: "New task" }).click();
  await page.getByRole("combobox", { name: "Task" }).fill(line);
  await page.getByRole("button", { name: "Add", exact: true }).click();
  await expect(page.getByRole("dialog")).toBeHidden();
}

async function place(page: Page, name: RegExp) {
  await page.getByRole("navigation", { name: "Places" }).getByRole("row", { name }).click();
}

async function addBlock(page: Page, title: string, start: string, minutes: number, kind?: "Break") {
  await page.getByRole("button", { name: "Add block" }).click();
  const form = page.getByRole("dialog", { name: "New Block" });
  await form.getByRole("textbox", { name: "Name", exact: true }).fill(title);
  await form.getByRole("textbox", { name: "Starts at" }).fill(start);
  await form.getByRole("textbox", { name: "Lasts, in minutes", exact: true }).fill(String(minutes));
  if (kind) {
    await form.getByRole("button", { name: /Kind/ }).click();
    await page.getByRole("option", { name: kind }).click();
  }
  await form.getByRole("button", { name: "Add", exact: true }).click();
  await expect(form).toBeHidden();
}

async function shot(page: Page, name: string) {
  await page.waitForTimeout(300);
  await page.screenshot({ path: `${OUT}/${name}.png`, fullPage: false });
}

for (const scheme of ["light", "dark"] as const) {
  for (const [width, height, size] of [[1280, 800, "desktop"], [390, 844, "phone"]] as const) {
    test(`${scheme} ${size}`, async ({ page }) => {
      test.setTimeout(240_000);
      await page.emulateMedia({ colorScheme: scheme });
      await page.setViewportSize({ width, height });
      const tag = (screen: string) => `${screen}-${scheme}-${size}`;
      await at(page, MORNING);
      await open(page);
      await shot(page, tag("01-empty"));

      await page.getByRole("button", { name: "New task" }).click();
      await page.getByRole("combobox", { name: "Task" }).fill("Call the bank tomorrow at 3pm p1 #");
      await shot(page, tag("02-quick-add"));
      await page.keyboard.press("Escape");
      await page.keyboard.press("Escape");

      await add(page, "Write the report tomorrow p1");
      await add(page, "Buy milk");
      await add(page, "Call the bank today at 3pm p2");
      await add(page, "Book the dentist next week");
      await add(page, "Water plants every day");
      const rows = page.getByRole("main").getByRole("row");
      await shot(page, tag("03-tasks"));

      await rows.filter({ hasText: "Write the report" }).first().click();
      await shot(page, tag("04-task-details"));

      await rows.filter({ hasText: "Buy milk" }).first().focus();
      await page.keyboard.press("Shift+F10");
      await shot(page, tag("05-row-menu"));
      await page.keyboard.press("Escape");

      await place(page, /^Today/);
      await addBlock(page, "Deep work", "9:00", 120);
      await addBlock(page, "Lunch", "10:30", 30, "Break");
      await addBlock(page, "Review", "11:00", 60);
      await addBlock(page, "Admin", "12:00", 60);
      await shot(page, tag("06-day"));

      await page.getByRole("button", { name: "Day actions" }).click();
      await shot(page, tag("07-day-actions"));
      await page.getByRole("menuitem", { name: "Replan from now" }).click();
      await expect(page.getByRole("dialog", { name: "Replan from now" })).toBeVisible();
      await shot(page, tag("08-replan"));
      await page.keyboard.press("Escape");

      await page.getByRole("button", { name: "Add block" }).click();
      await expect(page.getByRole("dialog", { name: "New Block" })).toBeVisible();
      await shot(page, tag("09-block-form"));
      await page.keyboard.press("Escape");

      await page.getByRole("button", { name: "Settings" }).click();
      await expect(page.getByRole("dialog", { name: "Settings" })).toBeVisible();
      await shot(page, tag("10-settings"));
      await page.getByRole("tab", { name: "Devices" }).click();
      await shot(page, tag("11-devices"));
      await page.keyboard.press("Escape");

      await page.getByRole("button", { name: "Keyboard shortcuts" }).click();
      await shot(page, tag("12-keys"));
      await page.keyboard.press("Escape");

      await place(page, /^Inbox/);
      await page.getByRole("main").getByRole("row").filter({ hasText: "Buy milk" }).first().focus();
      await page.keyboard.press("Shift+F10");
      await page.getByRole("menuitem", { name: "Move to trash" }).click();
      await place(page, /^Trash/);
      const trashed = page.getByRole("main").getByRole("row").filter({ hasText: "Buy milk" }).first();
      await trashed.focus();
      await page.keyboard.press("Delete");
      await shot(page, tag("13-confirm"));
    });
  }
}
