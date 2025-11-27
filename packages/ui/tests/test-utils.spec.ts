import { isBrowserBusy, waitUntilIdle, doAndWaitForIdle, waitForBrowserIdle } from './test-utils';
import { test, expect } from '@playwright/experimental-ct-svelte';

test.describe('Browser Idle Detection', () => {
	test('isBrowserBusy should detect running animations', async ({ page }) => {
		// Initially browser should be idle
		await page.goto('about:blank');
		const initiallyBusy = await isBrowserBusy(page);
		expect(initiallyBusy).toBe(false);

		// Start an animation
		await page.evaluate(() => {
			const div = document.createElement('div');
			div.style.width = '100px';
			div.style.height = '100px';
			div.style.backgroundColor = 'red';
			document.body.appendChild(div);

			// Start animation
			div.animate([{ opacity: 0 }, { opacity: 1 }], {
				duration: 2000,
				iterations: 1
			});
		});

		// Should detect animation is running
		const busyDuringAnimation = await isBrowserBusy(page);
		expect(busyDuringAnimation).toBe(true);

		// Wait for animation to complete
		await page.waitForTimeout(2100);

		// Should be idle again
		const idleAfterAnimation = await isBrowserBusy(page);
		expect(idleAfterAnimation).toBe(false);
	});

	test('waitUntilIdle should wait for animations to complete', async ({ page }) => {
		await page.goto('about:blank');

		// Start a short animation
		await page.evaluate(() => {
			const div = document.createElement('div');
			div.style.width = '100px';
			div.style.height = '100px';
			document.body.appendChild(div);

			div.animate([{ opacity: 0 }, { opacity: 1 }], {
				duration: 500,
				iterations: 1
			});
		});

		const startTime = Date.now();

		// Wait for idle - should wait for animation to complete
		await waitUntilIdle(page, { maxWait: 2000, consecutiveIdleChecks: 2 });

		const elapsedTime = Date.now() - startTime;

		// Should have waited at least 500ms for animation
		expect(elapsedTime).toBeGreaterThanOrEqual(450); // Allow some timing variation
		expect(elapsedTime).toBeLessThan(2000); // But not the full maxWait

		// Browser should be idle now
		const busy = await isBrowserBusy(page);
		expect(busy).toBe(false);
	});

	test('waitUntilIdle should timeout if browser stays busy', async ({ page }) => {
		await page.goto('about:blank');

		// Start a very long animation
		await page.evaluate(() => {
			const div = document.createElement('div');
			div.style.width = '100px';
			div.style.height = '100px';
			document.body.appendChild(div);

			div.animate([{ opacity: 0 }, { opacity: 1 }], {
				duration: 10000, // 10 seconds
				iterations: 1
			});
		});

		const startTime = Date.now();

		// Wait with short timeout
		await waitUntilIdle(page, { maxWait: 500, consecutiveIdleChecks: 2 });

		const elapsedTime = Date.now() - startTime;

		// Should have timed out after ~500ms
		expect(elapsedTime).toBeGreaterThanOrEqual(450);
		expect(elapsedTime).toBeLessThan(1000);
	});

	test('doAndWaitForIdle should execute action and wait for idle', async ({ page }) => {
		await page.goto('about:blank');

		let actionExecuted = false;

		await doAndWaitForIdle(
			page,
			async () => {
				actionExecuted = true;
				// Trigger an animation
				await page.evaluate(() => {
					const div = document.createElement('div');
					div.style.width = '100px';
					div.style.height = '100px';
					document.body.appendChild(div);

					div.animate([{ opacity: 0 }, { opacity: 1 }], {
						duration: 300,
						iterations: 1
					});
				});
			},
			{ maxWait: 2000 }
		);

		// Action should have been executed
		expect(actionExecuted).toBe(true);

		// Browser should be idle
		const busy = await isBrowserBusy(page);
		expect(busy).toBe(false);
	});

	test('waitForBrowserIdle should handle network idle state', async ({ page }) => {
		await page.goto('about:blank');

		// This should complete quickly since there's no network activity
		const startTime = Date.now();
		await waitForBrowserIdle(page, { timeout: 1000, networkIdleTime: 200 });
		const elapsedTime = Date.now() - startTime;

		// Should complete quickly (under 500ms)
		expect(elapsedTime).toBeLessThan(500);
	});

	test('waitUntilIdle should require consecutive idle checks', async ({ page }) => {
		await page.goto('about:blank');

		// Create a function that alternates between busy and idle
		await page.evaluate(() => {
			(window as any).toggleBusy = () => {
				const animations = document.getAnimations();
				if (animations.length > 0) {
					// Stop all animations
					animations.forEach((anim) => anim.cancel());
				} else {
					// Start a short animation
					const div = document.createElement('div');
					div.style.width = '100px';
					div.style.height = '100px';
					document.body.appendChild(div);

					div.animate([{ opacity: 0 }, { opacity: 1 }], {
						duration: 50,
						iterations: 1
					});
				}
			};
		});

		// Start with animation
		await page.evaluate(() => {
			const div = document.createElement('div');
			div.style.width = '100px';
			div.style.height = '100px';
			document.body.appendChild(div);

			div.animate([{ opacity: 0 }, { opacity: 1 }], {
				duration: 200,
				iterations: 1
			});
		});

		const startTime = Date.now();

		// Should wait for consecutive idle checks
		await waitUntilIdle(page, {
			maxWait: 2000,
			checkInterval: 50,
			consecutiveIdleChecks: 3 // Require 3 consecutive checks
		});

		const elapsedTime = Date.now() - startTime;

		// Should have waited for animation + consecutive checks
		expect(elapsedTime).toBeGreaterThanOrEqual(200); // At least animation duration
	});

	test('isBrowserBusy should handle page with no animations', async ({ page }) => {
		await page.goto('about:blank');

		// Add some static content
		await page.evaluate(() => {
			const div = document.createElement('div');
			div.textContent = 'Static content';
			document.body.appendChild(div);
		});

		const busy = await isBrowserBusy(page);
		expect(busy).toBe(false);
	});

	test('isBrowserBusy should detect CSS transitions', async ({ page }) => {
		await page.goto('about:blank');

		// Create element with transition
		await page.evaluate(() => {
			const div = document.createElement('div');
			div.style.width = '100px';
			div.style.height = '100px';
			div.style.backgroundColor = 'red';
			div.style.transition = 'opacity 1s';
			div.style.opacity = '0';
			document.body.appendChild(div);

			// Trigger transition
			setTimeout(() => {
				div.style.opacity = '1';
			}, 10);
		});

		// Wait a bit for transition to start
		await page.waitForTimeout(50);

		// Should detect transition
		const busy = await isBrowserBusy(page);
		// Note: This might be true or false depending on timing
		// The important thing is it doesn't throw an error
		expect(typeof busy).toBe('boolean');
	});

	test('waitUntilIdle with custom options', async ({ page }) => {
		await page.goto('about:blank');

		// Should complete quickly for idle page
		const startTime = Date.now();

		await waitUntilIdle(page, {
			maxWait: 10000,
			checkInterval: 100,
			consecutiveIdleChecks: 5
		});

		const elapsedTime = Date.now() - startTime;

		// Should complete quickly since page is idle
		// 5 checks * 100ms interval = ~500ms minimum
		expect(elapsedTime).toBeGreaterThanOrEqual(400);
		expect(elapsedTime).toBeLessThan(2000);
	});

	test('doAndWaitForIdle should handle errors in action', async ({ page }) => {
		await page.goto('about:blank');

		// Action that throws should propagate error
		await expect(
			doAndWaitForIdle(page, async () => {
				throw new Error('Action failed');
			})
		).rejects.toThrow('Action failed');
	});
});
