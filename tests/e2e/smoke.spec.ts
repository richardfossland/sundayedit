import { test, expect } from "@playwright/test";

// Smoke: the web frontend boots, the import screen renders, and loading the
// bundled demo project routes into the editor shell (sidebar + tabs).
test("boots to import screen and opens the demo project", async ({ page }) => {
  // Skip first-run onboarding so this exercises the import → editor path.
  await page.addInitScript(() =>
    localStorage.setItem("verbatim.onboarded", "1"),
  );
  await page.goto("/");

  // Import screen offers the demo entry point.
  const demo = page.getByRole("button", {
    name: /utforsk demo-prosjektet/i,
  });
  await expect(demo).toBeVisible();

  await demo.click();

  // The editor shell appears: sidebar nav buttons are labelled by title.
  await expect(page.getByRole("button", { name: "Editor" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Eksport" })).toBeVisible();
});
