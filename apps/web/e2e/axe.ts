import AxeBuilder from "@axe-core/playwright";
import type { Page } from "@playwright/test";

const AXE_TAGS = ["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"];

// Runs axe with the project's WCAG tag set. It first waits until nothing is
// animating, so a menu that is closing or a dialog that is opening cannot be
// caught half-transparent (which reads as a false contrast failure).
export async function analyzeAxe(page: Page) {
	await page.waitForFunction(() =>
		document.getAnimations().every((a) => a.playState !== "running"),
	);
	return new AxeBuilder({ page }).withTags(AXE_TAGS).analyze();
}
