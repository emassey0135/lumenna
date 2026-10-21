// The web client installed: it opens with no network, its store and all, and says what it is.

import { expect, test } from "@playwright/test";

test("the app opens with no network once it has been opened, its tasks with it", async ({ page, context }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Lumenna", level: 1 })).toBeVisible({ timeout: 30_000 });
  // Once the service worker holds the page, everything the app loads has been kept.
  await page.evaluate(async () => {
    await navigator.serviceWorker.ready;
  });
  await page.reload();
  await expect(page.getByRole("heading", { name: "Lumenna", level: 1 })).toBeVisible({ timeout: 30_000 });
  await page.getByRole("button", { name: "New task" }).click();
  await page.getByRole("combobox", { name: "Task" }).fill("water the plants");
  await page.getByRole("button", { name: "Add", exact: true }).click();
  await expect(page.getByRole("dialog")).toBeHidden();

  await context.setOffline(true);
  await page.reload();
  await expect(page.getByRole("heading", { name: "Lumenna", level: 1 })).toBeVisible({ timeout: 30_000 });
  await page.getByRole("navigation", { name: "Places" }).getByRole("row", { name: /^Tasks/ }).click();
  await expect(page.getByRole("main").getByRole("row", { name: /^water the plants/ })).toBeVisible();
});

test("the app says what it is to be installed", async ({ page }) => {
  await page.goto("/");
  const href = await page.locator('link[rel="manifest"]').getAttribute("href");
  expect(href).toBe("/manifest.webmanifest");
  const manifest = await (await page.request.get(href!)).json();
  expect(manifest.name).toBe("Lumenna");
  expect(manifest.display).toBe("standalone");
  for (const icon of manifest.icons) {
    expect((await page.request.get(icon.src)).ok(), icon.src).toBe(true);
  }
  expect(manifest.icons.map((icon: { sizes: string }) => icon.sizes)).toEqual(expect.arrayContaining(["192x192", "512x512"]));
});
