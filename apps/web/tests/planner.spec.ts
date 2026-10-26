// The planner: the day's own actions, Replan from Now and Plan the Day, a choice's reason, a
// task's history, and Carry Forward at a block's end — read back through the accessibility tree.
//
// The core runs in a worker, which Playwright's clock does not reach, so `at` sets the worker's
// clock too: its script is served with a `Date` running from a fixed morning in 2030, in this
// machine's zone. Nothing depends on when the tests run.

import { expect, test } from "./fixtures";
import type { Page } from "./fixtures";
import AxeBuilder from "@axe-core/playwright";

/** Runs the worker's clock (and, with `page`, the page's) from `time`. */
async function at(page: Page, time: Date, pageToo = false) {
  const target = time.getTime();
  await page.route(/\/src\/worker\.ts/, async (route) => {
    const response = await route.fetch();
    const shim =
      `{const Real=Date;const offset=${target}-Real.now();` +
      `class Shifted extends Real{constructor(...a){if(a.length)super(...a);else super(Real.now()+offset)}static now(){return Real.now()+offset}}` +
      `globalThis.Date=Shifted;}\n`;
    await route.fulfill({ response, body: shim + (await response.text()) });
  });
  if (pageToo) await page.clock.install({ time });
}

const MORNING = new Date(2030, 0, 15, 10, 40);

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
  await page.getByRole("button", { name: "New block" }).click();
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
  await expect(page.getByRole("treegrid", { name: "The day" }).getByRole("row", { name: new RegExp(title) })).toBeVisible();
}

/** Records everything the live region is given, since the announcer clears it again. */
async function record(page: Page) {
  await page.addInitScript(() => {
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

/** Deep work under way, Lunch overlapping it, then Review and Admin. */
async function crowdedDay(page: Page) {
  await place(page, /^Today/);
  await addBlock(page, "Deep work", "9:00", 120);
  await addBlock(page, "Lunch", "10:30", 30, "Break");
  await addBlock(page, "Review", "11:00", 60);
  await addBlock(page, "Admin", "12:00", 60);
}

test("a block added over one already in the day takes focus itself", async ({ page }) => {
  await at(page, MORNING);
  await open(page);
  await place(page, /^Today/);
  await addBlock(page, "Deep work", "9:00", 120);
  const day = page.getByRole("treegrid", { name: "The day" });
  await expect(day.getByRole("row", { name: /Deep work/ })).toBeFocused();
  await addBlock(page, "Lunch", "10:30", 30, "Break");
  await expect(day.getByRole("row", { name: /Lunch/ })).toBeFocused();
});

test("the day's own actions are offered on its summary row and in Day actions, in the core's order", async ({ page }) => {
  await at(page, MORNING);
  await open(page);
  await add(page, "Write report p1 30m");
  await crowdedDay(page);
  const day = page.getByRole("treegrid", { name: "The day" });
  const expected = ["Plan the day", "Replan from now", "What should I work on?", "End the day", "Clear the rest of the day"];
  await day.getByRole("row").first().click();
  await expect(day.getByRole("row").first()).toBeFocused();
  await page.keyboard.press("Shift+F10");
  await expect(page.getByRole("menuitem")).toHaveText(expected);
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Day actions" }).click();
  await expect(page.getByRole("menuitem")).toHaveText(expected);
  await axe(page);
  await page.keyboard.press("Escape");
});

test("What should I work on gives each choice's reason as its description, and starts the one picked", async ({ page }) => {
  await at(page, MORNING);
  await open(page);
  await add(page, "Write report p1 30m");
  await crowdedDay(page);
  await page.getByRole("button", { name: "Day actions" }).click();
  await page.getByRole("menuitem", { name: "What should I work on?" }).click();
  const picker = page.getByRole("dialog", { name: "What should I work on?" });
  await picker.getByRole("combobox").focus();
  const option = page.getByRole("option", { name: /^Write report/ });
  await expect(option).toHaveAccessibleDescription(/^Suggested because priority 1/);
  await expect(option).not.toHaveAccessibleName(/Suggested/);
  await option.click();
  await picker.getByRole("button", { name: "Start" }).click();
  await expect(picker).toBeHidden();
  await expect(page.getByRole("treegrid", { name: "The day" }).getByRole("row", { name: /^Write report.*in progress/ })).toBeVisible();
});

test("Replan from now says its proposal, makes the change checked, and shows what is proposed next", async ({ page }) => {
  await record(page);
  await at(page, MORNING);
  await open(page);
  await crowdedDay(page);
  await page.getByRole("button", { name: "Day actions" }).click();
  await page.getByRole("menuitem", { name: "Replan from now" }).click();
  const screen = page.getByRole("dialog", { name: "Replan from now" });
  await expect.poll(() => announced(page)).toContain(
    "Lunch moves from 10:30 AM to 11:00 AM; Review moves from 11:00 AM to 11:30 AM and shortens from 1 hour to 30 minutes.",
  );
  const boxes = screen.getByRole("checkbox", { name: "Make this change" });
  await expect(boxes).toHaveCount(2);
  await expect(boxes.first()).toBeFocused();
  await expect(boxes.first()).toBeChecked();
  await expect(boxes.first()).toHaveAccessibleDescription("Lunch moves from 10:30 AM to 11:00 AM");
  const apply = screen.getByRole("button", { name: "Make the changes checked" });
  await expect(apply).toHaveAccessibleDescription(/A change left unchecked is not made/);
  await axe(page);
  // Review's change turned down: Lunch's alone is made, and the day proposed again around it.
  await page.keyboard.press("Tab");
  await expect(boxes.nth(1)).toBeFocused();
  await page.keyboard.press("Space");
  await expect(boxes.nth(1)).not.toBeChecked();
  await apply.click();
  await expect.poll(() => announced(page)).toContainEqual(expect.stringMatching(/^Replanned: Lunch moves from 10:30 AM to 11:00 AM\./));
  await expect(screen).toBeVisible();
  await expect(boxes).toHaveCount(2);
  await expect(boxes.first()).toHaveAccessibleDescription("Lunch moves from 11:00 AM to 12:00 PM");
  await expect(boxes.first()).toBeFocused();
  await expect(boxes.nth(1)).toBeChecked();
  // The rest taken whole: nothing is left to propose, so the screen closes.
  await apply.click();
  await expect(screen).toBeHidden();
  await expect(page.getByRole("treegrid", { name: "The day" }).getByRole("row", { name: /Lunch/ })).toHaveAccessibleName(/12:00 PM/);
});

test("Replan from now offers other options, each by its sentence, and one taken is the proposal shown", async ({ page }) => {
  await record(page);
  await at(page, MORNING);
  await open(page);
  await crowdedDay(page);
  await page.getByRole("button", { name: "Day actions" }).click();
  await page.getByRole("menuitem", { name: "Replan from now" }).click();
  const screen = page.getByRole("dialog", { name: "Replan from now" });
  const others = screen.getByRole("region", { name: "Other options" });
  const skip = others.getByRole("button", { name: /^Skip Lunch today, your only break left today\./ });
  await expect(skip).toBeVisible();
  await axe(page);
  await skip.click();
  await expect.poll(() => announced(page)).toContain("Lunch is skipped today.");
  const boxes = screen.getByRole("checkbox", { name: "Make this change" });
  await expect(boxes).toHaveCount(1);
  await expect(boxes.first()).toHaveAccessibleDescription("Lunch is skipped today");
  await expect(boxes.first()).toBeFocused();
  // Nothing does better than nothing moving: no other options, and no heading for them.
  await expect(screen.getByRole("heading", { name: "Other options" })).toHaveCount(0);
  await screen.getByRole("button", { name: "Make the changes checked" }).click();
  await expect(screen).toBeHidden();
  await expect(page.getByRole("treegrid", { name: "The day" }).getByRole("row", { name: /Lunch/ })).toHaveCount(0);
});

test("Plan the day lists each sitting with why, and plans again around one turned down", async ({ page }) => {
  await record(page);
  await at(page, MORNING);
  await open(page);
  await add(page, "Write report p1 30m");
  await add(page, "Email p2 30m");
  await crowdedDay(page);
  await page.getByRole("button", { name: "Day actions" }).click();
  await page.getByRole("menuitem", { name: "Plan the day" }).click();
  const screen = page.getByRole("dialog", { name: "Plan the day" });
  await expect.poll(() => announced(page)).toContain("Plans 2 tasks: Write report and Email in Review.");
  const boxes = screen.getByRole("checkbox", { name: "Plan this" });
  await expect(boxes).toHaveCount(2);
  await expect(boxes.first()).toHaveAccessibleDescription(
    /^Write report goes in Review at 11:00 AM, for 30 minutes Suggested because priority 1/,
  );
  await axe(page);
  await boxes.nth(1).focus();
  await page.keyboard.press("Space");
  await expect(boxes.nth(1)).not.toBeChecked();
  await screen.getByRole("button", { name: "Plan the ones checked" }).click();
  await expect.poll(() => announced(page)).toContain("Planned 1 task. Now: Plans 1 task: Email in Admin.");
  await expect(boxes).toHaveCount(1);
  await expect(boxes.first()).toHaveAccessibleDescription(/^Email goes in Admin at 12:00 PM/);
  await screen.getByRole("button", { name: "Cancel" }).click();
  await expect(screen).toBeHidden();
  const day = page.getByRole("treegrid", { name: "The day" });
  await expect(day.getByRole("row", { name: /^Write report/ })).toBeVisible();
  await expect(day.getByRole("row", { name: /^Email/ })).toHaveCount(0);
});

test("a task's history is under its own heading on its screen, in the core's words, and never in its row", async ({ page }) => {
  await open(page);
  await add(page, "Water plants");
  const row = page.getByRole("main").getByRole("row", { name: /^Water plants/ });
  await row.click();
  const details = page.getByRole("complementary", { name: "Task details" });
  const history = details.getByRole("region", { name: "History" });
  await expect(history).toContainText("made");
  await expect(row).not.toHaveAccessibleName(/made|sitting/);
  await axe(page);
});

test("a block's end with work left short offers Carry Forward, which carries it to the next block", async ({ page }) => {
  // An hour and more of the app's clock, a tick a second: slower than a minute on an ARM
  // Windows machine, and under load.
  test.setTimeout(240_000);
  await record(page);
  await at(page, new Date(2030, 0, 15, 8, 50), true);
  await open(page);
  await add(page, "Write report");
  await place(page, /^Today/);
  await addBlock(page, "Deep work", "9:00", 60);
  await addBlock(page, "Afternoon", "14:00", 60);
  const day = page.getByRole("treegrid", { name: "The day" });
  await day.getByRole("row", { name: /Deep work/ }).click();
  await expect(day.getByRole("row", { name: /Deep work/ })).toBeFocused();
  await page.keyboard.press("Shift+F10");
  await page.getByRole("menuitem", { name: "Assign a task" }).click();
  await page.getByRole("combobox", { name: "Assign a task to Deep work" }).fill("Write");
  await page.getByRole("option", { name: /^Write report/ }).click();
  await page.getByRole("button", { name: "Assign" }).click();
  await page.getByRole("textbox", { name: "Planned length" }).fill("30m");
  await page.getByRole("dialog", { name: "Planned length" }).getByRole("button", { name: "Save" }).click();
  // Past Deep work's end, and the tab brought back, which asks what came due.
  await page.clock.runFor("01:11:00");
  await page.waitForTimeout(300);
  await page.evaluate(() => window.dispatchEvent(new Event("focus")));
  const due = page.getByRole("region", { name: "Reminders due" });
  const carry = due.getByRole("button", { name: "Carry forward" });
  await expect(carry).toBeVisible();
  await expect(carry).toHaveAccessibleDescription(/Carry the remaining 30 minutes to Afternoon/);
  await axe(page);
  await carry.click();
  await expect.poll(() => announced(page)).toContainEqual(expect.stringMatching(/^Carried forward Write report to Afternoon/));
  await expect(carry).toHaveCount(0);
  const afternoon = day.getByRole("row", { name: /Afternoon/ });
  await expect(afternoon).toHaveAccessibleName(/1 task assigned/);
});

test("a block's work outrunning its time is said with Replan, which opens Replan from now", async ({ page }) => {
  await record(page);
  await at(page, new Date(2030, 0, 15, 8, 50), true);
  await open(page);
  await add(page, "Write report");
  await place(page, /^Today/);
  await addBlock(page, "Deep work", "9:00", 60);
  // Straight after it, so the overrun has no free time to spill into.
  await addBlock(page, "Admin", "10:00", 60);
  const day = page.getByRole("treegrid", { name: "The day" });
  await day.getByRole("row", { name: /Deep work/ }).click();
  await page.keyboard.press("Shift+F10");
  await page.getByRole("menuitem", { name: "Assign a task" }).click();
  await page.getByRole("combobox", { name: "Assign a task to Deep work" }).fill("Write");
  await page.getByRole("option", { name: /^Write report/ }).click();
  await page.getByRole("button", { name: "Assign" }).click();
  await page.getByRole("textbox", { name: "Planned length" }).fill("90m");
  await page.getByRole("dialog", { name: "Planned length" }).getByRole("button", { name: "Save" }).click();
  // A plan too big from the start is said as the block starts.
  await page.clock.runFor("11:00");
  await page.waitForTimeout(300);
  await page.evaluate(() => window.dispatchEvent(new Event("focus")));
  const due = page.getByRole("region", { name: "Reminders due" });
  const replan = due.getByRole("button", { name: "Replan" });
  await expect(replan).toBeVisible();
  await expect(replan).toHaveAccessibleDescription(/^Deep work has .* more than it holds/);
  await axe(page);
  await replan.click();
  const screen = page.getByRole("dialog", { name: "Replan from now" });
  await expect(screen).toBeVisible();
  // Cancelled, the warning stays, and focus is back on its button.
  await screen.getByRole("button", { name: "Cancel" }).click();
  await expect(screen).toBeHidden();
  await expect(replan).toBeFocused();
});
