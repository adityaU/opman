/**
 * Driving the composer's engine picker from tests.
 *
 * Runner, model and agent used to be three controls with three interaction
 * shapes; every spec that configured a session repeated all three. They are one
 * control now, so the steps live here once.
 */
import { expect, type Page } from "@playwright/test";

export const ENGINE_CHIP = 'button[title="Choose runner, model, agent, effort, and permissions"]';

/** Open the palette, or no-op when it is already open. */
export async function openEnginePalette(page: Page) {
  const palette = page.locator('[role="dialog"][aria-label="Choose runner, model, and agent"]');
  if (await palette.isVisible().catch(() => false)) return palette;
  await page.locator(ENGINE_CHIP).click();
  await expect(palette).toBeVisible();
  return palette;
}

/**
 * Pick an option by its visible label. Substring matching by default: a model
 * row's accessible name carries its provider too ("Haiku 4.5 Anthropic"), so an
 * exact match on the model name alone would never hit.
 */
export async function pickEngineOption(page: Page, label: string, exact = false) {
  const palette = await openEnginePalette(page);
  await palette.getByRole("option", { name: label, exact }).first().click();
}

/** Choose a runner; the model and agent lists reload in place. */
export async function chooseRunner(page: Page, runner: string) {
  const labels: Record<string, string> = {
    opencode: "OpenCode",
    "claude-code": "Claude Code",
    claude: "Claude",
    codex: "Codex",
  };
  // Exact: a runner row's accessible name is just its label, and "Claude" is a
  // prefix of "Claude Code".
  await pickEngineOption(page, labels[runner] || runner, true);
}

/** Choose a model by name, closing the palette. */
export async function chooseModel(page: Page, modelName: string) {
  await pickEngineOption(page, modelName);
}

/** Set effort and permission, which live in the palette's footer. */
export async function setEngineSettings(
  page: Page,
  opts: { effort?: string; permission?: string },
) {
  const palette = await openEnginePalette(page);
  if (opts.effort) {
    const radio = palette.getByRole("radio", { name: opts.effort, exact: true });
    if (await radio.count()) await radio.click();
  }
  if (opts.permission) {
    await palette.getByLabel("Runner permissions").selectOption(opts.permission);
  }
}

export async function closeEnginePalette(page: Page) {
  const palette = page.locator('[role="dialog"][aria-label="Choose runner, model, and agent"]');
  if (await palette.isVisible().catch(() => false)) {
    await page.keyboard.press("Escape");
    await expect(palette).not.toBeVisible();
  }
}
