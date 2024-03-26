import * as ipc from '$lib/backend/ipc';
import { MenuBarManager } from '$lib/menuBarManager';
import { expect, test, describe, afterEach, vi, beforeEach } from 'vitest';

// Don't run tests as concurrent to avoid race conditions with shared state
describe('MenuBarManager', () => {
	beforeEach(() => {
		// Mock implementations because they depend on window
		vi.spyOn(ipc, 'listen').mockImplementation(async () => async () => undefined);
		vi.spyOn(ipc, 'invoke').mockImplementation(async () => undefined);
	});
	afterEach(() => {
		// Ensure fresh state for MenuBarManager
		MenuBarManager.instance = undefined;
	});

	describe('.getInstance', () => {
		test('It should return the same instance when called multiple times', () => {
			const first = MenuBarManager.getInstance();
			const second = MenuBarManager.getInstance();

			expect(first).toBe(second);
		});
	});

	describe('#setProjectId', () => {
		test('When called with a projectId; It should set the subscription', async () => {
			// @ts-expect-error testing lifecycle of private property
			MenuBarManager.getInstance().subscription = undefined;

			await MenuBarManager.getInstance().setProjectId('fooey');

			// @ts-expect-error testing lifecycle of private property
			expect(MenuBarManager.getInstance().subscription).toBeTruthy();
		});

		test('When called with a projectId and a subscription is already in place; It should unsubscribe the old subscription', async () => {
			const spy = vi.fn();
			// @ts-expect-error testing lifecycle of private property
			MenuBarManager.getInstance().subscription = spy;

			await MenuBarManager.getInstance().setProjectId('barey');

			expect(spy).toHaveBeenCalled();
		});

		test('When called with undefined and a subscription is already in place; It should unsubscribe the old subscription', async () => {
			const spy = vi.fn();
			// @ts-expect-error testing lifecycle of private property
			MenuBarManager.getInstance().subscription = spy;

			await MenuBarManager.getInstance().setProjectId(undefined);

			expect(spy).toHaveBeenCalled();
		});
	});
});
