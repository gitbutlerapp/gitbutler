/**
 * Test to verify that the persisted store correctly handles persistence
 * for the auto-update settings functionality
 */
import { persisted } from '@gitbutler/shared/persisted';
import { describe, test, expect, beforeEach } from 'vitest';

describe('persisted store integration', () => {
	beforeEach(() => {
		// Clear localStorage before each test
		localStorage.clear();
	});

	test('should persist disableAutoChecks setting', () => {
		const store1 = persisted(false, 'disableAutoUpdateChecks');

		// Set to true
		store1.set(true);

		// Check that it was stored in localStorage
		expect(localStorage.getItem('disableAutoUpdateChecks')).toBe('true');

		// Create a new store instance with the same key
		const store2 = persisted(false, 'disableAutoUpdateChecks');

		// Subscribe to get the value
		let storedValue: boolean;
		store2.subscribe((value) => {
			storedValue = value;
		});

		// Should have retrieved the stored value
		expect(storedValue!).toBe(true);
	});

	test('should handle default value when no stored value exists', () => {
		const store = persisted(false, 'nonExistentKey');

		let value: boolean;
		store.subscribe((v) => {
			value = v;
		});

		// Should use the default value
		expect(value!).toBe(false);
	});
});
