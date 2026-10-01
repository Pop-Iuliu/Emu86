import { expect, test } from "@playwright/test";
import path from "node:path";
import { Buffer } from "node:buffer";
import { fileURLToPath } from "node:url";

const specDir = path.dirname(fileURLToPath(import.meta.url));
const programBin = (name: string) => path.join(specDir, "..", "..", "tests", "programs", name);

test("smoke: load demo, watch memory count down to zero, observe halt", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByText("Load a program to begin.")).toBeVisible();

  await page.getByRole("button", { name: "Load demo" }).click();
  await expect(page.getByTestId("reg-ax")).toHaveText(/AX.*0000/);
  await expect(page.getByTestId("program-info")).toHaveText(/countdown\.bin.*22 bytes.*FFFF:0000/);

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByTestId("reg-bx")).toHaveText(/BX.*0020/);

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByTestId("reg-ax")).toHaveText(/AX.*0003/);

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByTestId("reg-cx")).toHaveText(/CX.*FFFF/);

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByTestId("mem-byte-32")).toHaveText("03");
  await expect(page.getByTestId("mem-byte-33")).toHaveText("00");

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByTestId("mem-byte-32")).toHaveText("02");
  await expect(page.getByTestId("last-step")).toContainText("mem 00020");

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByText("FLAGS = F002")).toBeVisible();

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByText("FFFF:000B")).toBeVisible();

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByTestId("mem-byte-32")).toHaveText("01");

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByText("FLAGS = F002")).toBeVisible();

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByText("FFFF:000B")).toBeVisible();

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByTestId("mem-byte-32")).toHaveText("00");
  await expect(page.getByText("FLAGS = F046")).toBeVisible();

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByText("FFFF:0011")).toBeVisible();
  await expect(page.getByText("FLAGS = F046")).toBeVisible();

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByText("FFFF:0013")).toBeVisible();

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByTestId("reg-cx")).toHaveText(/CX.*0000/);

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByTestId("halted")).toBeVisible();
  await expect(page.getByText(/IP advanced past HLT/)).toBeVisible();
  await expect(page.getByRole("button", { name: "Step" })).toBeDisabled();
});

test("load a raw .bin, execute it, and replay after reset without reselecting the file", async ({
  page,
}) => {
  await page.goto("/");
  await page.setInputFiles(`[data-testid="binary-input"]`, programBin("carry.bin"));

  await expect(page.getByTestId("program-info")).toHaveText(/carry\.bin.*7 bytes.*FFFF:0000/);
  await expect(page.getByTestId("reg-ax")).toHaveText(/AX.*0000/);
  await expect(page.getByText("FLAGS = F002")).toBeVisible();

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByTestId("reg-ax")).toHaveText(/AX.*FFFF/);

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByTestId("reg-ax")).toHaveText(/AX.*0000/);
  await expect(page.getByText("FLAGS = F057")).toBeVisible();

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByTestId("halted")).toBeVisible();
  await expect(page.getByRole("button", { name: "Step" })).toBeDisabled();

  await page.getByRole("button", { name: "Reset" }).click();
  await expect(page.getByTestId("halted")).toHaveCount(0);
  await expect(page.getByTestId("reg-ax")).toHaveText(/AX.*0000/);
  await expect(page.getByText("FLAGS = F002")).toBeVisible();
  await expect(page.getByTestId("program-info")).toHaveText(/carry\.bin.*7 bytes.*FFFF:0000/);
  await expect(page.getByRole("button", { name: "Step" })).toBeEnabled();

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByTestId("reg-ax")).toHaveText(/AX.*FFFF/);
});

test("flags drive a counted loop loaded from a raw .bin", async ({ page }) => {
  await page.goto("/");
  await page.setInputFiles(`[data-testid="binary-input"]`, programBin("loop.bin"));
  await expect(page.getByTestId("program-info")).toHaveText(/loop\.bin.*20 bytes.*FFFF:0000/);
  await expect(page.getByTestId("reg-ax")).toHaveText(/AX.*0000/);

  for (let i = 0; i < 15; i++) {
    await page.getByRole("button", { name: "Step" }).click();
  }

  await expect(page.getByTestId("reg-ax")).toHaveText(/AX.*0006/);
  await expect(page.getByTestId("reg-cx")).toHaveText(/CX.*0000/);
  await expect(page.getByText("FLAGS = F046")).toBeVisible();
  await expect(page.getByTestId("halted")).toBeVisible();
  await expect(page.getByRole("button", { name: "Step" })).toBeDisabled();
});

test("invalid binaries are rejected and keep the current program", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Load demo" }).click();
  await expect(page.getByTestId("program-info")).toHaveText(/countdown\.bin/);

  await page.setInputFiles(`[data-testid="binary-input"]`, {
    name: "too-big.bin",
    mimeType: "application/octet-stream",
    buffer: Buffer.alloc(0x100001),
  });
  await expect(page.getByText(/address space holds at most/)).toBeVisible();
  await expect(page.getByTestId("program-info")).toHaveText(/countdown\.bin/);

  await page.setInputFiles(`[data-testid="binary-input"]`, {
    name: "empty.bin",
    mimeType: "application/octet-stream",
    buffer: Buffer.alloc(0),
  });
  await expect(page.getByText(/program file is empty/)).toBeVisible();
  await expect(page.getByTestId("program-info")).toHaveText(/countdown\.bin/);

  await page.getByRole("button", { name: "Step" }).click();
  await expect(page.getByTestId("reg-bx")).toHaveText(/BX.*0020/);
});
