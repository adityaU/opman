import { test, expect, Page } from "@playwright/test";
import {
  setupMockAPI,
  SESSION_ID,
  MOCK_MESSAGES,
} from "./helpers";

/**
 * Mock messages that include:
 *   1. An assistant message with a fenced code block (exercises CodeBlock in message-turn)
 *   2. A tool call whose output is markdown with a fenced code block (exercises ToolOutput markdown path)
 *   3. A tool call with JSON input (exercises ToolInput JSON rendering)
 */
const FONT_TEST_MESSAGES = [
  // User message
  {
    info: { role: "user", messageID: "msg_font_001", time: 1700000100 },
    parts: [{ type: "text", text: "Show me some code" }],
    metadata: {},
  },
  // Assistant message with fenced code block
  {
    info: {
      role: "assistant",
      messageID: "msg_font_002",
      time: 1700000200,
      model: { modelID: "claude-sonnet-4-20250514", providerID: "anthropic" },
    },
    parts: [
      {
        type: "text",
        text: "Here is an example:\n\n```javascript\nconst hello = 'world';\nconsole.log(hello);\n```\n\nAnd some `inline code` too.",
      },
    ],
    metadata: {
      cost: 0.001,
      tokens: { input: 10, output: 20, reasoning: 0, cache_read: 0, cache_write: 0 },
    },
  },
  // Assistant message with a tool call that produces markdown output
  {
    info: {
      role: "assistant",
      messageID: "msg_font_003",
      time: 1700000300,
      model: { modelID: "claude-sonnet-4-20250514", providerID: "anthropic" },
    },
    parts: [
      {
        type: "tool",
        tool: "web_search",
        callID: "tc_font_001",
        state: {
          input: { query: "search for files" },
          output:
            '<task_result>\nFound these results:\n\n```python\ndef search(query):\n    return db.find(query)\n```\n\nThe function is in `src/search.py`.\n</task_result>',
          status: "completed",
          time: { start: 1700000300, end: 1700000400 },
        },
      },
      {
        type: "tool",
        tool: "read_file",
        callID: "tc_font_002",
        state: {
          input: { path: "/src/index.ts" },
          output: 'export const VERSION = "1.0.0";',
          status: "completed",
          time: { start: 1700000400, end: 1700000500 },
        },
      },
    ],
    metadata: {
      cost: 0.002,
      tokens: { input: 50, output: 100, reasoning: 0, cache_read: 0, cache_write: 0 },
    },
  },
];

/**
 * Set up the mock API with our font-test messages instead of the default ones.
 */
async function setupFontTestPage(page: Page) {
  await setupMockAPI(page);

  // Override the messages endpoint with our custom messages
  await page.route(`**/api/session/${SESSION_ID}/messages*`, (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        messages: FONT_TEST_MESSAGES,
        total: FONT_TEST_MESSAGES.length,
      }),
    })
  );

  // Navigate and authenticate
  await page.goto("/");
  await page.evaluate(() => {
    sessionStorage.setItem("opman_token", "mock-jwt-token");
  });
  await page.reload();
  await page.waitForSelector(".chat-layout", { timeout: 10_000 });
}

/**
 * Check that a computed font-family contains a monospace font.
 * We look for common monospace font names or the generic "monospace" keyword.
 */
function assertMonospace(fontFamily: string, context: string) {
  const lower = fontFamily.toLowerCase();
  const hasMonospace =
    lower.includes("monospace") ||
    lower.includes("ibm plex mono") ||
    lower.includes("jetbrains mono") ||
    lower.includes("fira code") ||
    lower.includes("consolas") ||
    lower.includes("menlo") ||
    lower.includes("courier");
  expect(hasMonospace, `Expected monospace font for ${context}, got: "${fontFamily}"`).toBe(true);
}

// ─────────────────────────────────────────────────────
// Font rendering tests
// ─────────────────────────────────────────────────────

test.describe("Markdown code font rendering", () => {
  test.beforeEach(async ({ page }) => {
    await setupFontTestPage(page);
  });

  test("message-turn fenced code block uses monospace font", async ({ page }) => {
    // Wait for the code-block-wrapper to appear (from the assistant message)
    const codeBlock = page.locator(".code-block-wrapper").first();
    await expect(codeBlock).toBeVisible({ timeout: 10_000 });

    // Check the code element inside the code block body
    const codeEl = codeBlock.locator(".code-block-body code").first();
    await expect(codeEl).toBeVisible();

    const fontFamily = await codeEl.evaluate(
      (el) => window.getComputedStyle(el).fontFamily
    );
    assertMonospace(fontFamily, "message-turn fenced code block <code>");
  });

  test("message-turn inline code uses monospace font", async ({ page }) => {
    const inlineCode = page.locator(".inline-code").first();
    await expect(inlineCode).toBeVisible({ timeout: 10_000 });

    const fontFamily = await inlineCode.evaluate(
      (el) => window.getComputedStyle(el).fontFamily
    );
    assertMonospace(fontFamily, "message-turn inline code");
  });

  test("tool-call output markdown code block uses monospace font", async ({ page }) => {
    // Expand the task tool call to show output
    const toolCalls = page.locator(".tool-call");
    // The first tool-call is the "task" tool — click its header to expand
    const taskToolHeader = toolCalls.first().locator(".tool-call-header");
    await expect(taskToolHeader).toBeVisible({ timeout: 10_000 });
    await taskToolHeader.click();

    // Wait for the tool output markdown container
    const toolMarkdown = page.locator(".tool-output-markdown").first();
    await expect(toolMarkdown).toBeVisible({ timeout: 5_000 });

    // The markdown should contain a code block rendered by shared CodeBlock
    // which wraps in .code-block-wrapper
    const codeBlockInTool = toolMarkdown.locator(".code-block-wrapper");

    if (await codeBlockInTool.count() > 0) {
      // Shared CodeBlock was used — check the code element inside
      const codeEl = codeBlockInTool.first().locator("code").first();
      await expect(codeEl).toBeVisible();

      const fontFamily = await codeEl.evaluate(
        (el) => window.getComputedStyle(el).fontFamily
      );
      assertMonospace(fontFamily, "tool-output markdown fenced code (CodeBlock)");
    } else {
      // Fallback: check any code element inside the tool markdown
      const codeEl = toolMarkdown.locator("code").first();
      await expect(codeEl).toBeVisible();

      const fontFamily = await codeEl.evaluate(
        (el) => window.getComputedStyle(el).fontFamily
      );
      assertMonospace(fontFamily, "tool-output markdown fenced code (fallback)");
    }
  });

  test("tool-call JSON input uses monospace font", async ({ page }) => {
    // The read_file tool should have JSON input
    const toolCalls = page.locator(".tool-call");
    // Click on all tool headers to expand them
    const headers = toolCalls.locator(".tool-call-header");
    const count = await headers.count();
    for (let i = 0; i < count; i++) {
      await headers.nth(i).click();
    }

    // Wait briefly for expansion
    await page.waitForTimeout(500);

    // Find a SyntaxHighlighter-rendered element with language class
    const syntaxCode = page.locator('code[class*="language-"]').first();
    if (await syntaxCode.isVisible()) {
      const fontFamily = await syntaxCode.evaluate(
        (el) => window.getComputedStyle(el).fontFamily
      );
      assertMonospace(fontFamily, "tool-call JSON input code");
    }
  });

  test("code-block line numbers use monospace font", async ({ page }) => {
    const lineNumbers = page.locator(".code-block-line-numbers").first();
    await expect(lineNumbers).toBeVisible({ timeout: 10_000 });

    const fontFamily = await lineNumbers.evaluate(
      (el) => window.getComputedStyle(el).fontFamily
    );
    assertMonospace(fontFamily, "code-block line numbers");
  });

  test("screenshot of rendered code blocks", async ({ page }) => {
    // Take a screenshot for visual verification
    await page.screenshot({
      path: "tests/screenshots/markdown-code-font.png",
      fullPage: true,
    });
  });
});
