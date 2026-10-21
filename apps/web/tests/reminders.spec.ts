// Reminders: a task's or block's own list, and what the open tab gives as each comes due — read
// back through the accessibility tree. Delivery runs on the page's clock, set to a fixed morning
// in 2030 in the zone this machine runs in, so nothing depends on when the tests run.

import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

async function open(page: Page) {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Lumenna", level: 1 })).toBeVisible({ timeout: 30_000 });
}

async function place(page: Page, name: RegExp) {
  await page.getByRole("navigation", { name: "Places" }).getByRole("row", { name }).click();
}

async function addBlock(page: Page, title: string, at: string, day?: string) {
  await page.getByRole("button", { name: "Add block" }).click();
  const form = page.getByRole("dialog", { name: "New Block" });
  await form.getByRole("textbox", { name: "Name", exact: true }).fill(title);
  if (day) await form.getByRole("textbox", { name: "Day", exact: true }).fill(day);
  await form.getByRole("textbox", { name: "Starts at" }).fill(at);
  await form.getByRole("textbox", { name: "Lasts, in minutes", exact: true }).fill("60");
  await form.getByRole("button", { name: "Add", exact: true }).click();
  await expect(form).toBeHidden();
}

/**
 * Moves the page's clock on by `time`, then brings the tab back to the front, which asks what
 * came due: the timer's own look may still be waiting on the core from the start of the run.
 */
async function later(page: Page, time: string) {
  await page.clock.runFor(time);
  await page.waitForTimeout(300);
  await page.evaluate(() => window.dispatchEvent(new Event("focus")));
}

/**
 * Records, rather than shows, each notification, as a browser that allows them; and records
 * everything the live region is given, since the announcer clears it again on the clock these
 * tests move on.
 */
async function record(page: Page) {
  await page.addInitScript(() => {
    const shown: { title: string; body?: string; tag?: string }[] = [];
    (window as unknown as { shownNotifications: typeof shown }).shownNotifications = shown;
    class Recorded {
      static permission = "granted";
      static requestPermission = async () => "granted";
      onclick: (() => void) | null = null;
      constructor(title: string, options?: NotificationOptions) {
        shown.push({ title, body: options?.body, tag: options?.tag });
      }
      close() {}
    }
    (window as unknown as { Notification: unknown }).Notification = Recorded;
    const announced: string[] = [];
    (window as unknown as { announced: string[] }).announced = announced;
    new MutationObserver((changes) => {
      for (const change of changes) {
        if ((change.target as Element).closest?.("[aria-live]")) {
          for (const node of change.addedNodes) if (node.textContent) announced.push(node.textContent);
        }
      }
    }).observe(document, { childList: true, subtree: true });
  });
}

function announced(page: Page): Promise<string[]> {
  return page.evaluate(() => (window as unknown as { announced: string[] }).announced);
}

async function axe(page: Page) {
  const results = await new AxeBuilder({ page }).analyze();
  expect(results.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target).join(", ")}`)).toEqual([]);
}

test("a block's reminders are listed, a phrase the core cannot read stays in its dialog, and one is removed", async ({ page }) => {
  await open(page);
  await place(page, /^Blocks/);
  await addBlock(page, "Deep work", "9am");
  await page.getByRole("treegrid", { name: "Blocks" }).getByRole("row", { name: /^Deep work/ }).focus();
  await page.keyboard.press("Shift+F10");
  await page.getByRole("menuitem", { name: "Reminders" }).click();
  const defaults = page.getByRole("dialog", { name: "1 reminder, the defaults" });
  await expect(defaults.getByText("5 minutes before it starts, a default")).toBeVisible();
  await axe(page);
  await defaults.getByRole("button", { name: "Add reminder" }).click();
  const field = page.getByRole("dialog", { name: "New reminder for Deep work" }).getByRole("textbox", { name: "When" });
  await field.fill("when it feels right");
  await field.press("Enter");
  await expect(field).toHaveAttribute("aria-invalid", "true");
  await expect(field).toHaveAccessibleDescription(/is not a reminder/);
  await field.fill("10m after end");
  await field.press("Enter");
  const own = page.getByRole("dialog", { name: "1 reminder", exact: true });
  await expect(own.getByText("10 minutes after it ends", { exact: true })).toBeVisible();
  const remove = own.getByRole("button", { name: "Remove reminder" });
  await expect(remove).toHaveAccessibleDescription("10 minutes after it ends");
  await remove.click();
  await expect(page.getByRole("dialog", { name: "1 reminder, the defaults" })).toBeVisible();
  await expect(page.getByRole("dialog", { name: "1 reminder, the defaults" }).getByRole("button", { name: "Add reminder" })).toBeFocused();
});

test("a task with no due date offers reminders at a moment of its own", async ({ page }) => {
  await open(page);
  await page.getByRole("button", { name: "New task" }).click();
  await page.getByRole("combobox", { name: "Task" }).fill("someday");
  await page.getByRole("button", { name: "Add", exact: true }).click();
  await expect(page.getByRole("dialog")).toBeHidden();
  await page.getByRole("main").getByRole("row", { name: /^someday/ }).focus();
  await page.keyboard.press("Shift+F10");
  await page.getByRole("menuitem", { name: "Reminders" }).click();
  await expect(page.getByRole("dialog", { name: "No reminders" })).toBeVisible();
  // Said as it opens, so it reaches a screen reader, and shown on the status line.
  await expect(page.getByRole("contentinfo")).toContainText("a moment of its own");
});

test("a reminder due while the tab is open is said, shown as a notification, and dismissed there", async ({ page }) => {
  await record(page);
  await page.clock.install({ time: new Date(2030, 0, 15, 8, 50) });
  await open(page);
  await place(page, /^Today/);
  await addBlock(page, "Deep work", "9:00", "2030-01-15");
  await later(page, "06:00");
  const said = "Deep work starts in 5 minutes — nothing assigned yet";
  await expect.poll(() => announced(page)).toContain(said);
  const shown = await page.evaluate(() => (window as unknown as { shownNotifications: unknown[] }).shownNotifications);
  expect(shown).toEqual([expect.objectContaining({ title: "Deep work", body: said })]);
  const due = page.getByRole("region", { name: "Reminders due" });
  await expect(due.getByText(said)).toBeVisible();
  await expect(due.getByRole("button", { name: "Snooze 10 minutes" })).toBeVisible();
  await axe(page);
  await due.getByRole("button", { name: "Dismiss" }).click();
  await expect(due).toHaveCount(0);
  await expect.poll(() => announced(page)).toContain("Dismissed");
  // Given once: the next look has nothing new.
  await later(page, "01:00");
  await page.waitForTimeout(300);
  expect(await page.evaluate(() => (window as unknown as { shownNotifications: unknown[] }).shownNotifications)).toHaveLength(1);
});

test("with reminders off in this browser nothing is said, and a default the core refuses stays at its field", async ({ page }) => {
  await record(page);
  await page.clock.install({ time: new Date(2030, 0, 15, 8, 50) });
  await open(page);
  await place(page, /^Today/);
  await addBlock(page, "Deep work", "9:00", "2030-01-15");
  await page.getByRole("button", { name: "Settings" }).click();
  const settings = page.getByRole("dialog", { name: "Settings" });
  await settings.getByRole("checkbox", { name: "Reminders on this device" }).focus();
  await page.keyboard.press("Space");
  await expect(settings.getByRole("checkbox", { name: "Reminders on this device" })).not.toBeChecked();
  const field = settings.getByRole("textbox", { name: "Reminders for a block" });
  await field.fill("at the end of time");
  await field.press("Enter");
  await expect(field).toHaveAttribute("aria-invalid", "true");
  await expect(field).toHaveAccessibleDescription(/is not a reminder/);
  await expect(field).toHaveValue("at the end of time");
  await field.fill("5m before start, at end");
  await field.press("Enter");
  await expect(field).not.toHaveAttribute("aria-invalid", "true");
  await settings.getByRole("button", { name: "Close" }).click();
  await later(page, "06:00");
  await page.waitForTimeout(300);
  expect((await announced(page)).filter((text) => text.startsWith("Deep work starts"))).toEqual([]);
  await expect(page.getByRole("region", { name: "Reminders due" })).toHaveCount(0);
});
