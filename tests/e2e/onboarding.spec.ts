import { test, expect } from "@playwright/test";

// Fresh first run (empty localStorage → onboarding shows): walk the steps and
// land in the editor via the demo project.
test("first-run onboarding walks through to the editor", async ({ page }) => {
  await page.goto("/");

  await expect(
    page.getByRole("heading", { name: /velkommen til sundayedit/i }),
  ).toBeVisible();

  await page.getByRole("button", { name: "Kom i gang" }).click();
  await page.getByRole("button", { name: "Hopp over" }).click(); // skip profile
  await page.getByRole("button", { name: "Fortsett" }).click(); // past model step

  await page.getByRole("button", { name: /utforsk demo-prosjektet/i }).click();

  await expect(page.getByRole("button", { name: "Editor" })).toBeVisible();
});
