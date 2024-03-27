import * as ipc from '$lib/backend/ipc';
import { MenuBarController } from '$lib/menuBarController';
import { expect, test, describe, afterEach, vi, beforeEach } from 'vitest';

// Don't run tests as concurrent to avoid race conditions with shared state
describe('MenuBarController', () => {
	beforeEach(() => {
		// Mock implementations because they depend on window
		vi.spyOn(ipc, 'listen').mockImplementation(async () => async () => undefined);
		vi.spyOn(ipc, 'invoke').mockImplementation(async () => undefined);
	});
	afterEach(() => {
		// Ensure fresh state for MenuBarManager
		MenuBarController.instance = undefined;
	});

	describe('.getInstance', () => {
		test('It should return the same instance when called multiple times', () => {
			const first = MenuBarController.getInstance();
			const second = MenuBarController.getInstance();

			expect(first).toBe(second);
		});
	});

	describe('#setProjectId', () => {
		test('When called with a projectId; It should set the subscription', async () => {
			// @ts-expect-error testing lifecycle of private property
			MenuBarController.getInstance().subscription = undefined;

			await MenuBarController.getInstance().setProjectId('fooey');

			// @ts-expect-error testing lifecycle of private property
			expect(MenuBarController.getInstance().subscription).toBeTruthy();
		});

		test('When called with a projectId; It should enable the project settings button', async () => {
			// @ts-expect-error testing lifecycle of private property
			MenuBarController.getInstance().subscription = undefined;

			const spy = vi
				.spyOn(MenuBarController.getInstance(), 'setProjectSettingsEnabled')
				.mockImplementation(async () => undefined);

			await MenuBarController.getInstance().setProjectId('fooey');

			expect(spy).toHaveBeenCalledOnce();
			expect(spy).toHaveBeenCalledWith(true);
		});

		test('When called with a projectId and a subscription is already in place; It should unsubscribe the old subscription', async () => {
			const spy = vi.fn();
			// @ts-expect-error testing lifecycle of private property
			MenuBarController.getInstance().subscription = spy;

			await MenuBarController.getInstance().setProjectId('barey');

			expect(spy).toHaveBeenCalled();
		});

		test('When called with a projectId and a subscription is already in place; It should disable the project settings button then re-enable it', async () => {
			await MenuBarController.getInstance().setProjectId('bar');

			const spy = vi
				.spyOn(MenuBarController.getInstance(), 'setProjectSettingsEnabled')
				.mockImplementation(async () => undefined);

			await MenuBarController.getInstance().setProjectId('fooey');

			expect(spy).toHaveBeenCalledTimes(2);
			expect(spy).toHaveBeenNthCalledWith(1, false);
			expect(spy).toHaveBeenNthCalledWith(2, true);
		});

		test('When called with undefined and a subscription is already in place; It should unsubscribe the old subscription', async () => {
			const spy = vi.fn();
			// @ts-expect-error testing lifecycle of private property
			MenuBarController.getInstance().subscription = spy;

			await MenuBarController.getInstance().setProjectId(undefined);

			expect(spy).toHaveBeenCalled();
		});

		test('When called with undefined and a subscription is already in place; It should disable the project settings button', async () => {
			await MenuBarController.getInstance().setProjectId('bar');

			const spy = vi
				.spyOn(MenuBarController.getInstance(), 'setProjectSettingsEnabled')
				.mockImplementation(async () => undefined);

			await MenuBarController.getInstance().setProjectId(undefined);

			expect(spy).toHaveBeenCalledOnce();
			expect(spy).toHaveBeenCalledWith(false);
		});
	});
});
