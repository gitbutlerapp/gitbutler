import { getBaseBranchData, getRemoteBranches } from './mock/baseBranch';
import { MOCK_GIT_HEAD, MOCK_OPEN_WORKSPACE_MODE } from './mock/mode';
import { getProject, isGetProjectArgs, listProjects } from './mock/projects';
import { getSecret, isGetSecretArgs } from './mock/secrets';
import { MOCK_APP_SETTINGS } from './mock/settings';
import { MOCK_USER } from './mock/user';
import { invoke, type InvokeArgs } from '@tauri-apps/api/core';

function mockInternals(window: any) {
	window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ ?? {};
	window.__TAURI_OS_PLUGIN_INTERNALS__ = window.__TAURI_OS_PLUGIN_INTERNALS__ ?? {};
}

type MockCallback = (command: string, args?: InvokeArgs) => unknown;

export function mockIPC(window: any, cb: MockCallback): void {
	mockInternals(window);

	window.__TAURI_INTERNALS__.transformCallback = function transformCallback(
		callback?: (response: any) => void,
		once = false
	) {
		const identifier = window.crypto.getRandomValues(new Uint32Array(1))[0];
		const prop = `_${identifier}`;

		Object.defineProperty(window, prop, {
			value: (result: any) => {
				if (once) {
					Reflect.deleteProperty(window, prop);
				}

				return callback && callback(result);
			},
			writable: false,
			configurable: true
		});

		return identifier;
	};

	window.__TAURI_INTERNALS__.invoke = async function (
		cmd: string,
		args?: InvokeArgs
	): Promise<unknown> {
		return cb(cmd, args);
	} as typeof invoke;
}

export function mockWindows(window: any, current: string, ..._additionalWindows: string[]): void {
	mockInternals(window);
	window.__TAURI_INTERNALS__.metadata = {
		currentWindow: { label: current },
		currentWebview: { windowLabel: current, label: current }
	};
}

export function mockPlatform(window: any, platform: string): void {
	mockInternals(window);
	window.__TAURI_INTERNALS__.platform = platform;
}

export function clearMocks(window: any): void {
	if (window.__TAURI_INTERNALS__) {
		window.__TAURI_INTERNALS__ = undefined;
		delete window.__TAURI_INTERNALS__;
	}

	if (window.__TAURI_OS_PLUGIN_INTERNALS__) {
		window.__TAURI_OS_PLUGIN_INTERNALS__ = undefined;
		delete window.__TAURI_OS_PLUGIN_INTERNALS__;
	}
}

function raiseInvalidArgumentsError(command: string, args: unknown): never {
	throw new Error('Invalid arguments for ' + command + ': ' + JSON.stringify(args));
}

function raiseMissingMockError(command: string): never {
	throw new Error('Missing mock for command: ' + command);
}

Cypress.on('window:before:load', (win) => {
	mockPlatform(win, 'macos');
	mockWindows(win, 'main');
	mockIPC(win, async (command, args) => {
		switch (command) {
			case 'operating_mode':
				return MOCK_OPEN_WORKSPACE_MODE;
			case 'set_project_active':
				// Do nothing
				return await Promise.resolve();
			case 'fetch_from_remotes':
				// Do nothing
				return await Promise.resolve();
			case 'git_head':
				return MOCK_GIT_HEAD;
			case 'get_base_branch_data':
				return getBaseBranchData();
			case 'git_remote_branches':
				return getRemoteBranches();
			case 'secret_get_global':
				if (!isGetSecretArgs(args)) {
					return raiseInvalidArgumentsError(command, args);
				}
				return getSecret(args);
			case 'get_project':
				if (!isGetProjectArgs(args)) {
					return raiseInvalidArgumentsError(command, args);
				}
				return getProject(args);
			case 'list_projects':
				return listProjects();
			case 'plugin:updater|check':
				return null;
			case 'get_user':
				return MOCK_USER;
			case 'plugin:window|theme':
				return 'light';
			case 'get_app_settings':
				return MOCK_APP_SETTINGS;
			case 'plugin:event|listen':
				return await Promise.resolve({});
			case 'plugin:store|load':
				return await Promise.resolve({});
			case 'plugin:path|resolve_directory':
				return await Promise.resolve({});
			default:
				return raiseMissingMockError(command);
		}
	});
});

declare global {
	namespace Cypress {
		interface Chainable {
			/**
			 * Mock the Tauri IPC calls.
			 * @param cb - The callback to handle the IPC calls.
			 */
			mockIPC(cb: MockCallback): void;

			/**
			 * Clear all mocks.
			 */
			clearMocks(): void;
		}
	}
}

Cypress.Commands.add('mockIPC', (cb: MockCallback) => {
	cy.window().then((win) => {
		mockIPC(win, cb);
	});
});

Cypress.Commands.add('clearMocks', () => {
	cy.window().then((win) => {
		clearMocks(win);
	});
});
