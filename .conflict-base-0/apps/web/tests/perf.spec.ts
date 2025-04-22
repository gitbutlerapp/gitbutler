import { test, expect } from '@playwright/test';

const limits = {
	timeToFirstByte: 800,
	domContentLoaded: 2000,
	loadComplete: 3000,
	firstContentfulPaint: 1500
};

test.describe('Performance Tests', () => {
	test('should keep performanceMetrics under limits', async ({ page }) => {
		await page.setExtraHTTPHeaders({ Accept: 'text/html' });

		const response = await page.goto('/');
		expect(response?.status()).toBe(200);

		// Get performance metrics using modern APIs
		const performanceMetrics = await page.evaluate(() => {
			const navigation = performance.getEntriesByType(
				'navigation'
			)[0] as PerformanceNavigationTiming;
			const paintEntries = performance.getEntriesByType('paint');
			const firstPaint = paintEntries.find((entry) => entry.name === 'first-paint');
			const firstContentfulPaint = paintEntries.find(
				(entry) => entry.name === 'first-contentful-paint'
			);

			return {
				// Navigation timing metrics
				domContentLoaded: navigation.domContentLoadedEventEnd - navigation.startTime,
				loadComplete: navigation.loadEventEnd - navigation.startTime,
				timeToFirstByte: navigation.responseStart - navigation.requestStart,
				domInteractive: navigation.domInteractive - navigation.startTime,

				// Paint timing metrics
				firstPaint: firstPaint ? firstPaint.startTime : 0,
				firstContentfulPaint: firstContentfulPaint ? firstContentfulPaint.startTime : 0
			};
		});

		// Assert on the metrics
		expect(performanceMetrics.timeToFirstByte).toBeLessThan(limits.timeToFirstByte);
		expect(performanceMetrics.domContentLoaded).toBeLessThan(limits.domContentLoaded);
		expect(performanceMetrics.loadComplete).toBeLessThan(limits.loadComplete);
		expect(performanceMetrics.firstContentfulPaint).toBeLessThan(limits.firstContentfulPaint);
	});
});
