import { persisted, setStorageItem } from '@gitbutler/shared/persisted';
import { get } from 'svelte/store';
import { assert, test, describe, beforeEach } from 'vitest';

const TEST_KEY = 'test-key';
const TEST_VALUE = 'test-value';
describe('persisted store', () => {
	beforeEach(() => {
		window.localStorage.clear();
	});

	test('initial value', async () => {
		const store = persisted(TEST_VALUE, TEST_KEY);
		assert.equal(get(store), TEST_VALUE);
	});

	test('updated value', async () => {
		const store = persisted<string | undefined>(undefined, TEST_KEY);
		assert.equal(get(store), undefined);

		store.set(TEST_VALUE);
		assert.equal(get(store), TEST_VALUE);

		const anotherStore = persisted<string | undefined>(undefined, TEST_KEY);
		assert.equal(get(anotherStore), TEST_VALUE);
	});

	test('stored value', async () => {
		setStorageItem(TEST_KEY, TEST_VALUE);
		const store = persisted<string | undefined>(undefined, TEST_KEY);
		assert.equal(get(store), TEST_VALUE);
	});
});
