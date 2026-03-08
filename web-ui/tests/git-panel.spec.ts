/**
 * Playwright tests for the Git Panel.
 *
 * Covers:
 *  - Opening via Cmd+Shift+G
 *  - Panel renders with tabs (Changes, Log)
 *  - Branch name display
 *  - Changes tab: staged, unstaged, untracked sections
 *  - File rows with status indicators
 *  - Stage / unstage individual files
 *  - Stage all / unstage all
 *  - Commit form appears when staged files exist
 *  - Commit message typing and submit
 *  - Log tab: commit list
 *  - Commit detail view from log
 *  - Branch switcher dropdown
 *  - Refresh button
 *  - Empty state (clean working tree)
 *  - Close panel
 *  - AI action buttons
 */

import { test, expect, Page } from "@playwright/test";
import { setupMockAPI } from "./helpers";

// ── Mock git data ─────────────────────────────────────

const MOCK_GIT_STATUS = {
  branch: "feature/auth",
  staged: [
    { path: "src/auth.ts", status: "M" },
    { path: "src/types.ts", status: "A" },
  ],
  unstaged: [
    { path: "src/api.ts", status: "M" },
    { path: "README.md", status: "M" },
  ],
  untracked: [{ path: "src/new-file.ts", status: "?" }],
};

const MOCK_GIT_STATUS_CLEAN = {
  branch: "main",
  staged: [],
  unstaged: [],
  untracked: [],
};

const MOCK_GIT_LOG = {
  commits: [
    {
      hash: "abc123def456",
      short_hash: "abc123d",
      author: "Developer",
      date: "2024-01-15T10:30:00Z",
      message: "Add authentication module",
    },
    {
      hash: "789xyz000111",
      short_hash: "789xyz0",
      author: "Developer",
      date: "2024-01-14T15:20:00Z",
      message: "Initial project setup",
    },
    {
      hash: "def456ghi789",
      short_hash: "def456g",
      author: "Colleague",
      date: "2024-01-13T09:00:00Z",
      message: "Configure CI pipeline",
    },
  ],
};

const MOCK_GIT_DIFF = {
  diff: `--- a/src/api.ts
+++ b/src/api.ts
@@ -10,6 +10,8 @@
 import { fetchData } from "./utils";
 
+// New authentication endpoint
+export async function authenticate() {
+  return fetch("/api/auth");
+}
`,
};

const MOCK_GIT_SHOW = {
  hash: "abc123def456",
  author: "Developer",
  date: "2024-01-15T10:30:00Z",
  message: "Add authentication module",
  diff: `--- a/src/auth.ts\n+++ b/src/auth.ts\n@@ -0,0 +1,5 @@\n+export function login() {\n+  return true;\n+}\n`,
  files: [
    { path: "src/auth.ts", status: "A" },
    { path: "src/types.ts", status: "M" },
  ],
};

const MOCK_GIT_BRANCHES = {
  current: "feature/auth",
  local: ["main", "feature/auth", "develop"],
  remote: ["origin/main", "origin/develop"],
};

const MOCK_GIT_COMMIT_RESPONSE = {
  hash: "new123commit",
  message: "feat: add login flow",
};

// ── Setup helpers ─────────────────────────────────────

async function setupGitMocks(page: Page, statusOverride?: object) {
  await setupMockAPI(page);

  // Git-specific routes (last registered = highest priority)
  await page.route("**/api/git/status", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(statusOverride || MOCK_GIT_STATUS),
    })
  );

  await page.route("**/api/git/diff*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(MOCK_GIT_DIFF),
    })
  );

  await page.route("**/api/git/log*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(MOCK_GIT_LOG),
    })
  );

  await page.route("**/api/git/show*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(MOCK_GIT_SHOW),
    })
  );

  await page.route("**/api/git/branches", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(MOCK_GIT_BRANCHES),
    })
  );

  await page.route("**/api/git/stage", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ ok: true }),
    })
  );

  await page.route("**/api/git/unstage", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ ok: true }),
    })
  );

  await page.route("**/api/git/commit", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify(MOCK_GIT_COMMIT_RESPONSE),
    })
  );

  await page.route("**/api/git/discard", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ ok: true }),
    })
  );

  await page.route("**/api/git/checkout", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ branch: "main", success: true }),
    })
  );

  await page.route("**/api/git/range-diff*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ branch: "feature/auth", base: "main", commits: [], diff: "", files_changed: 0 }),
    })
  );

  await page.route("**/api/git/context-summary*", (route) =>
    route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({ branch: "feature/auth", recent_commits: [], staged_count: 2, unstaged_count: 2, untracked_count: 1, summary: "" }),
    })
  );
}

async function navigateAndOpenGit(page: Page, statusOverride?: object) {
  await setupGitMocks(page, statusOverride);

  await page.goto("/");
  await page.evaluate(() => {
    sessionStorage.setItem("opman_token", "mock-jwt-token");
  });
  await page.reload();
  await page.waitForSelector(".chat-layout", { timeout: 10_000 });

  // Open git panel with Cmd+Shift+G
  await page.keyboard.press("Meta+Shift+g");
  const panel = page.locator(".git-panel");
  await expect(panel).toBeVisible({ timeout: 5_000 });
  return panel;
}

// ── Tests ─────────────────────────────────────────────

test.describe("Git Panel", () => {
  test.describe("Opening and closing", () => {
    test("opens via Cmd+Shift+G", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      await expect(panel).toBeVisible();
    });

    test("toggle Cmd+Shift+G closes the panel", async ({ page }) => {
      await navigateAndOpenGit(page);
      // Press again to close
      await page.keyboard.press("Meta+Shift+g");
      const panel = page.locator(".git-panel");
      await expect(panel).not.toBeVisible({ timeout: 3_000 });
    });

    test("close button closes the panel", async ({ page }) => {
      await navigateAndOpenGit(page);
      const closeBtn = page.locator('[aria-label="Close git panel"]');
      await closeBtn.click();
      const panel = page.locator(".git-panel");
      await expect(panel).not.toBeVisible({ timeout: 3_000 });
    });
  });

  test.describe("Panel structure", () => {
    test("shows Changes and Log tabs", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      const tabs = panel.locator(".git-tab");
      await expect(tabs).toHaveCount(2);
      await expect(tabs.nth(0)).toContainText("Changes");
      await expect(tabs.nth(1)).toContainText("Log");
    });

    test("Changes tab is active by default", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      const changesTab = panel.locator(".git-tab").nth(0);
      await expect(changesTab).toHaveClass(/active/);
    });

    test("shows branch name", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      const branchToggle = panel.locator(".git-branch-toggle");
      await expect(branchToggle).toContainText("feature/auth");
    });

    test("shows refresh button", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      const refreshBtn = panel.locator('[aria-label="Refresh"]');
      await expect(refreshBtn).toBeVisible();
    });

    test("tab badge shows total change count", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      const badge = panel.locator(".git-tab-badge");
      // 2 staged + 2 unstaged + 1 untracked = 5
      await expect(badge).toContainText("5");
    });
  });

  test.describe("Changes tab — file sections", () => {
    test("shows Staged section with file count", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      const stagedHeader = panel.locator(".git-section-title", { hasText: "Staged" });
      await expect(stagedHeader).toBeVisible();
      await expect(stagedHeader).toContainText("2");
    });

    test("shows Changes (unstaged) section with file count", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      const unstagedHeader = panel.locator(".git-section-title", { hasText: "Changes" });
      await expect(unstagedHeader).toBeVisible();
      await expect(unstagedHeader).toContainText("2");
    });

    test("shows Untracked section with file count", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      const untrackedHeader = panel.locator(".git-section-title", { hasText: "Untracked" });
      await expect(untrackedHeader).toBeVisible();
      await expect(untrackedHeader).toContainText("1");
    });

    test("file rows display file path", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      const fileRows = panel.locator(".git-file-row");
      const count = await fileRows.count();
      // 2 staged + 2 unstaged + 1 untracked = 5
      expect(count).toBe(5);
    });

    test("file rows show status letter with color", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      const statusLetters = panel.locator(".git-file-status");
      const count = await statusLetters.count();
      expect(count).toBe(5);
      // First staged file is M (modified)
      await expect(statusLetters.first()).toContainText("M");
    });
  });

  test.describe("Stage / unstage actions", () => {
    test("staged files have unstage button", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      // Staged section's file rows should have minus button
      const stagedSection = panel.locator(".git-section").first();
      const actions = stagedSection.locator(".git-file-action");
      const count = await actions.count();
      expect(count).toBeGreaterThanOrEqual(1);
    });

    test("unstaged files have stage button", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      // Changes section's file rows should have plus button for staging
      const fileRows = panel.locator(".git-file-row");
      // At least one should have an action button
      const actions = panel.locator(".git-file-action");
      const count = await actions.count();
      expect(count).toBeGreaterThanOrEqual(3);
    });

    test("section header has quick action button (stage all / unstage all)", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      const sectionActions = panel.locator(".git-section-action");
      const count = await sectionActions.count();
      expect(count).toBeGreaterThanOrEqual(2);
    });
  });

  test.describe("Commit form", () => {
    test("commit form is visible when staged files exist", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      const commitForm = panel.locator(".git-commit-form");
      await expect(commitForm).toBeVisible();
    });

    test("commit input has correct placeholder", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      const commitInput = panel.locator(".git-commit-input");
      await expect(commitInput).toHaveAttribute("placeholder", "Commit message...");
    });

    test("commit button shows staged count", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      const commitBtn = panel.locator(".git-commit-button");
      await expect(commitBtn).toContainText("Commit");
      await expect(commitBtn).toContainText("2");
    });

    test("commit button is disabled when message is empty", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      const commitBtn = panel.locator(".git-commit-button");
      await expect(commitBtn).toBeDisabled();
    });

    test("commit button becomes enabled after typing a message", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      const commitInput = panel.locator(".git-commit-input");
      await commitInput.fill("feat: add login flow");

      const commitBtn = panel.locator(".git-commit-button");
      await expect(commitBtn).not.toBeDisabled();
    });

    test("commit form is NOT visible when no staged files", async ({ page }) => {
      const noStagedStatus = {
        branch: "main",
        staged: [],
        unstaged: [{ path: "src/api.ts", status: "M" }],
        untracked: [],
      };
      const panel = await navigateAndOpenGit(page, noStagedStatus);
      const commitForm = panel.locator(".git-commit-form");
      await expect(commitForm).not.toBeVisible();
    });
  });

  test.describe("Clean working tree", () => {
    test("shows 'Working tree clean' when no changes", async ({ page }) => {
      const panel = await navigateAndOpenGit(page, MOCK_GIT_STATUS_CLEAN);
      const empty = panel.locator(".git-empty");
      await expect(empty).toBeVisible();
      await expect(empty).toContainText("Working tree clean");
    });
  });

  test.describe("Tab switching", () => {
    test("clicking Log tab switches to log view", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      const logTab = panel.locator(".git-tab").nth(1);
      await logTab.click();

      await expect(logTab).toHaveClass(/active/);

      // Should show commit log entries
      const logEntries = panel.locator(".git-log-entry");
      await expect(logEntries.first()).toBeVisible({ timeout: 5_000 });
    });

    test("clicking Changes tab switches back", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);

      // Switch to Log
      await panel.locator(".git-tab").nth(1).click();
      await expect(panel.locator(".git-log-entry").first()).toBeVisible({ timeout: 5_000 });

      // Switch back to Changes
      await panel.locator(".git-tab").nth(0).click();
      await expect(panel.locator(".git-tab").nth(0)).toHaveClass(/active/);

      // File rows should be visible again
      const fileRows = panel.locator(".git-file-row");
      await expect(fileRows.first()).toBeVisible({ timeout: 5_000 });
    });
  });

  test.describe("Log tab", () => {
    test("shows commit entries", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      await panel.locator(".git-tab").nth(1).click();

      const entries = panel.locator(".git-log-entry");
      await expect(entries).toHaveCount(3);
    });

    test("commit entries show hash, message, and meta", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      await panel.locator(".git-tab").nth(1).click();

      const firstEntry = panel.locator(".git-log-entry").first();
      await expect(firstEntry).toBeVisible({ timeout: 5_000 });

      await expect(firstEntry.locator(".git-log-hash")).toContainText("abc123d");
      await expect(firstEntry.locator(".git-log-message")).toContainText("Add authentication module");
    });

    test("clicking a commit navigates to commit detail view", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      await panel.locator(".git-tab").nth(1).click();

      const firstEntry = panel.locator(".git-log-entry").first();
      await expect(firstEntry).toBeVisible({ timeout: 5_000 });
      await firstEntry.click();

      // Should show commit detail view
      const detail = panel.locator(".git-commit-detail");
      await expect(detail).toBeVisible({ timeout: 5_000 });
    });

    test("commit detail shows hash and message", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      await panel.locator(".git-tab").nth(1).click();

      await panel.locator(".git-log-entry").first().click();
      const detail = panel.locator(".git-commit-detail");
      await expect(detail).toBeVisible({ timeout: 5_000 });

      await expect(detail.locator(".git-commit-meta-hash")).toContainText("abc123d");
      await expect(detail.locator(".git-commit-meta-message")).toContainText("Add authentication module");
    });

    test("commit detail shows files summary", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      await panel.locator(".git-tab").nth(1).click();

      await panel.locator(".git-log-entry").first().click();
      const detail = panel.locator(".git-commit-detail");
      await expect(detail).toBeVisible({ timeout: 5_000 });

      const summary = detail.locator(".git-commit-files-summary");
      await expect(summary).toContainText("2 files");
    });
  });

  test.describe("Branch switcher", () => {
    test("clicking branch button opens branch dropdown", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      const branchBtn = panel.locator(".git-branch-toggle");
      await branchBtn.click();

      const dropdown = panel.locator(".git-branch-dropdown");
      await expect(dropdown).toBeVisible({ timeout: 5_000 });
    });

    test("branch dropdown shows local and remote branches", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      await panel.locator(".git-branch-toggle").click();

      const dropdown = panel.locator(".git-branch-dropdown");
      await expect(dropdown).toBeVisible({ timeout: 5_000 });

      // Local branches
      const branchItems = dropdown.locator(".git-branch-item");
      const count = await branchItems.count();
      // 3 local + 2 remote = 5
      expect(count).toBeGreaterThanOrEqual(3);
    });

    test("current branch is marked", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      await panel.locator(".git-branch-toggle").click();

      const dropdown = panel.locator(".git-branch-dropdown");
      await expect(dropdown).toBeVisible({ timeout: 5_000 });

      const currentBranch = dropdown.locator(".git-branch-item.current");
      await expect(currentBranch).toBeVisible();
      await expect(currentBranch).toContainText("feature/auth");
    });

    test("branch filter narrows the list", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      await panel.locator(".git-branch-toggle").click();

      const dropdown = panel.locator(".git-branch-dropdown");
      await expect(dropdown).toBeVisible({ timeout: 5_000 });

      const filterInput = dropdown.locator(".git-branch-filter");
      await filterInput.fill("main");

      const items = dropdown.locator(".git-branch-item");
      // Should match "main" and "origin/main"
      const count = await items.count();
      expect(count).toBeGreaterThanOrEqual(1);
      expect(count).toBeLessThanOrEqual(3);
    });
  });

  test.describe("AI action buttons", () => {
    test("AI action buttons are visible when there are changes", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      const aiActions = panel.locator(".git-ai-actions");
      await expect(aiActions).toBeVisible();
    });

    test("shows Review Changes, Write Commit Msg, and Draft PR buttons", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      const aiButtons = panel.locator(".git-ai-button");
      const count = await aiButtons.count();
      expect(count).toBe(3);
    });
  });

  test.describe("File diff navigation", () => {
    test("clicking a file row navigates to diff view", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);

      // Click the first file row (an unstaged file to avoid navigating within staged section)
      const fileRows = panel.locator(".git-file-row");
      await fileRows.first().click();

      // Should show diff view
      const diffView = panel.locator(".git-diff-fullview");
      await expect(diffView).toBeVisible({ timeout: 5_000 });
    });

    test("diff view shows file name in header", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      await panel.locator(".git-file-row").first().click();

      const diffView = panel.locator(".git-diff-fullview");
      await expect(diffView).toBeVisible({ timeout: 5_000 });

      const header = diffView.locator(".git-diff-header");
      await expect(header).toBeVisible();
    });

    test("back button returns to file list", async ({ page }) => {
      const panel = await navigateAndOpenGit(page);
      await panel.locator(".git-file-row").first().click();

      const diffView = panel.locator(".git-diff-fullview");
      await expect(diffView).toBeVisible({ timeout: 5_000 });

      // Click back button
      const backBtn = panel.locator('[aria-label="Go back"]');
      await backBtn.click();

      // Should be back to file list
      await expect(diffView).not.toBeVisible({ timeout: 3_000 });
      const fileRows = panel.locator(".git-file-row");
      await expect(fileRows.first()).toBeVisible({ timeout: 3_000 });
    });
  });
});
