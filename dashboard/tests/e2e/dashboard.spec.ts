import { test, expect } from '@playwright/test';

test.describe('GraphRAG Dashboard E2E', () => {
  test.beforeEach(async ({ page }) => {
    // Navigate to the root URL (configured in playwright.config.ts)
    await page.goto('/');
  });

  test('should display the title card', async ({ page }) => {
    // Assert title exists
    await expect(page.locator('.title-card h1')).toHaveText('GraphRAG Topology');
  });

  test('should switch modes between Force and DAG', async ({ page }) => {
    // Grab the buttons
    const forceBtn = page.getByRole('button', { name: /Force Physics/i });
    const dagBtn = page.getByRole('button', { name: /DAG Hierarchy/i });

    // Validate default state
    await expect(forceBtn).toHaveClass(/active/);
    await expect(dagBtn).not.toHaveClass(/active/);

    // Switch to DAG
    await dagBtn.click();
    await expect(dagBtn).toHaveClass(/active/);
    await expect(forceBtn).not.toHaveClass(/active/);
  });

  test('should render the 3D canvas without crashing', async ({ page }) => {
    // react-force-graph-3d places a single <canvas> element inside its container
    const canvas = page.locator('canvas');
    await expect(canvas).toBeVisible(); // Check if the 3D context loaded into the DOM
  });
});
