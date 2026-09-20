import { expect, test } from "@playwright/test";

test("smoke: load, step through the demo program, observe halt", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByText("Load a program to begin.")).toBeVisible();

  await page.getByRole("button", { name: "Load" }).click();
  await expect(page.getByTestId("reg-ax")).toHaveText(/AX.*0000/);

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByTestId("reg-bx")).toHaveText(/BX.*0020/);

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByTestId("reg-ax")).toHaveText(/AX.*BEEF/);

  await page.getByRole("button", { name: "Step" }).click();
  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByText("FLAGS = F096")).toBeVisible();

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByText("FLAGS = F093")).toBeVisible();

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByTestId("reg-cx")).toHaveText(/CX.*D001/);

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByTestId("halted")).toBeVisible();

  await expect(page.getByRole("button", { name: "Step" })).toBeDisabled();
});
