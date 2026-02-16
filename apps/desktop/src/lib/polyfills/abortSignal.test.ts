import { polyfillAbortSignalTimeout } from '$lib/polyfills/abortSignal';
import { describe, expect, it, beforeEach, afterEach } from 'vitest';

describe('polyfillAbortSignalTimeout', () => {
	let originalTimeout: typeof AbortSignal.timeout | undefined;

	beforeEach(() => {
		// Save the original timeout method if it exists
		originalTimeout = AbortSignal.timeout;
	});

	afterEach(() => {
		// Restore the original timeout method
		if (originalTimeout) {
			AbortSignal.timeout = originalTimeout;
		} else {
			// @ts-expect-error - Deleting a static method for testing
			delete AbortSignal.timeout;
		}
	});

	it('should add AbortSignal.timeout if it does not exist', () => {
		// Remove the timeout method to simulate an environment without it
		// @ts-expect-error - Deleting a static method for testing
		delete AbortSignal.timeout;

		polyfillAbortSignalTimeout();

		expect(AbortSignal.timeout).toBeDefined();
		expect(typeof AbortSignal.timeout).toBe('function');
	});

	it('should not override existing AbortSignal.timeout', () => {
		function mockTimeout() {
			return new AbortController().signal;
		}
		AbortSignal.timeout = mockTimeout;

		polyfillAbortSignalTimeout();

		expect(AbortSignal.timeout).toBe(mockTimeout);
	});

	it('should create an AbortSignal that aborts after the specified timeout', async () => {
		// Remove the timeout method to test the polyfill implementation
		// @ts-expect-error - Deleting a static method for testing
		delete AbortSignal.timeout;

		polyfillAbortSignalTimeout();

		const signal = AbortSignal.timeout(100);

		expect(signal).toBeInstanceOf(AbortSignal);
		expect(signal.aborted).toBe(false);

		// Wait for the timeout to trigger
		await new Promise((resolve) => setTimeout(resolve, 150));

		expect(signal.aborted).toBe(true);
	});

	it('should set the abort reason to TimeoutError', async () => {
		// Remove the timeout method to test the polyfill implementation
		// @ts-expect-error - Deleting a static method for testing
		delete AbortSignal.timeout;

		polyfillAbortSignalTimeout();

		const signal = AbortSignal.timeout(100);

		// Wait for the timeout to trigger
		await new Promise((resolve) => setTimeout(resolve, 150));

		expect(signal.reason).toBeInstanceOf(DOMException);
		expect(signal.reason.name).toBe('TimeoutError');
	});

	it('should allow listening to abort events', async () => {
		// Remove the timeout method to test the polyfill implementation
		// @ts-expect-error - Deleting a static method for testing
		delete AbortSignal.timeout;

		polyfillAbortSignalTimeout();

		const signal = AbortSignal.timeout(100);
		let abortEventFired = false;

		signal.addEventListener('abort', () => {
			abortEventFired = true;
		});

		// Wait for the timeout to trigger
		await new Promise((resolve) => setTimeout(resolve, 150));

		expect(abortEventFired).toBe(true);
	});
});
