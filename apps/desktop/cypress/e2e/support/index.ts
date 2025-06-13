import { getBaseBranchData, getRemoteBranches } from './mock/baseBranch';
import { MOCK_BRANCH_LISTINGS } from './mock/branches';
import { MOCK_TREE_CHANGES, MOCK_UNIFIED_DIFF } from './mock/changes';
import { MOCK_GIT_HEAD, MOCK_OPEN_WORKSPACE_MODE } from './mock/mode';
import { getProject, isGetProjectArgs, listProjects } from './mock/projects';
import { getSecret, isGetSecretArgs } from './mock/secrets';
import { MOCK_APP_SETTINGS } from './mock/settings';
import { MOCK_STACK_DETAILS, MOCK_STACKS } from './mock/stacks';
import { MOCK_BRANCH_STATUSES_RESPONSE } from './mock/upstreamIntegration';
import { MOCK_USER } from './mock/user';
import { MOCK_VIRTUAL_BRANCHES } from './mock/virtualBranches';
import { MOCK_WORKTREE_CHANGES } from './mock/worktree';
// Sorry about this. For now it's ok for the tests.
// eslint-disable-next-line no-relative-import-paths/no-relative-import-paths
import { TestId } from '../../../src/lib/testing/testIds';
import { invoke, type InvokeArgs } from '@tauri-apps/api/core';

function mockInternals(window: any) {
	window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ ?? {};
	window.__TAURI_OS_PLUGIN_INTERNALS__ = window.__TAURI_OS_PLUGIN_INTERNALS__ ?? {};
}

type MockCallback = (args?: InvokeArgs) => unknown;
type MockCommandCallback = (command: string, args?: InvokeArgs) => unknown;

export function mockIPC(window: any, cb: MockCommandCallback): void {
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

const ipcMocks = new Map<string, MockCallback>();

Cypress.on('window:before:load', (win) => {
	mockPlatform(win, 'macos');
	mockWindows(win, 'main');
	mockIPC(win, async (command, args) => {
		if (ipcMocks.has(command)) {
			return ipcMocks.get(command)!(args);
		}

		switch (command) {
			case 'tree_change_diffs':
				return MOCK_UNIFIED_DIFF;
			case 'git_get_global_config':
				return await Promise.resolve(undefined);
			case 'commit_details':
				return MOCK_TREE_CHANGES;
			case 'stack_details':
				return MOCK_STACK_DETAILS;
			case 'changes_in_worktree':
				return MOCK_WORKTREE_CHANGES;
			case 'list_branches':
				return MOCK_BRANCH_LISTINGS;
			case 'list_virtual_branches':
				return MOCK_VIRTUAL_BRANCHES;
			case 'upstream_integration_statuses':
				return MOCK_BRANCH_STATUSES_RESPONSE;
			case 'stacks':
				return MOCK_STACKS;
			case 'operating_mode':
				return MOCK_OPEN_WORKSPACE_MODE;
			case 'set_project_active':
				// Do nothing
				return await Promise.resolve(true);
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
			case 'update_telemetry_distinct_id':
				return await Promise.resolve();

			case 'plugin:updater|check':
				return null;
			case 'get_user':
				return MOCK_USER;
			case 'plugin:window|theme':
				return 'light';
			case 'get_app_settings':
				return MOCK_APP_SETTINGS;
			case 'plugin:event|unlisten':
				return await Promise.resolve({});
			case 'plugin:event|listen':
				return await Promise.resolve({});
			case 'plugin:store|load':
				return await Promise.resolve({});
			case 'plugin:store|get':
				return await Promise.resolve([undefined, false]);
			case 'plugin:path|resolve_directory':
				return await Promise.resolve({});
			case 'plugin:log|log':
				return await Promise.resolve({});
			default:
				return raiseMissingMockError(command);
		}
	});
});

Cypress.on('window:before:unload', (win) => {
	clearMocks(win);
});

type TestIdValues = `${TestId}`;

declare global {
	namespace Cypress {
		interface Chainable {
			/**
			 * Get an element by its data-testid attribute.
			 *
			 * @param testId - The data-testid value to search for.
			 * @param containingText - Optional text content to filter the elements by.
			 */
			getByTestId(testId: TestIdValues, containingText?: string): Chainable<JQuery<HTMLElement>>;
			/**
			 * Get an element by its data-* attribute value.
			 *
			 * @param testId - The data-testid value to search for.
			 * @param containingText - Optional text content to filter the elements by.
			 */
			getByTestIdByValue(testId: TestIdValues, withValue: string): Chainable<JQuery<HTMLElement>>;
			/**
			 * Clear all mocks.
			 */
			clearMocks(): void;
			/**
			 * Highlight the text in a given element.
			 */
			selectText(element: Cypress.Chainable<JQuery<HTMLElement>>): void;
			/**
			 *
			 */
			urlMatches(pattern: string): void;
		}
	}
}

export function mockCommand(command: string, cb: MockCallback) {
	ipcMocks.set(command, cb);
}

export function clearCommandMocks() {
	ipcMocks.clear();
}

Cypress.Commands.add('clearMocks', () => {
	cy.window().then((win) => {
		clearMocks(win);
	});
});

Cypress.Commands.add('getByTestId', (testId: TestIdValues, containingText?: string) => {
	if (containingText) {
		return cy.contains(`[data-testid="${testId}"]`, containingText, { timeout: 15000 });
	}
	return cy.get(`[data-testid="${testId}"]`, { timeout: 15000 });
});

Cypress.Commands.add('getByTestIdByValue', (testId: TestIdValues, withValue: string) => {
	return cy.get(`[data-testid-${testId}="${withValue}"]`, { timeout: 15000 }).first();
});

Cypress.Commands.add('selectText', (element: Cypress.Chainable<JQuery<HTMLElement>>) => {
	element
		.trigger('mousedown')
		.then(($el) => {
			const el = $el[0];
			if (!el) {
				throw new Error(`Element could not be resolved: ${element}`);
			}
			const document = el.ownerDocument;
			const range = document.createRange();
			range.selectNodeContents(el);
			document.getSelection()?.removeAllRanges();
			document.getSelection()?.addRange(range);
		})
		.trigger('mouseup');
	cy.document().trigger('selectionchange');
});

Cypress.on('uncaught:exception', () => {
	// Returning false here prevents Cypress from
	// failing the test.
	return false;
});

beforeEach(() => {
	cy.viewport('macbook-11');
});

Cypress.Commands.add('urlMatches', (pattern: string) => {
	cy.url({ timeout: 10000 }).should('include', pattern);
});
