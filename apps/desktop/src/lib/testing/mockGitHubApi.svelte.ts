import createBackend from '$lib/backend';
import { GitHubClient } from '$lib/forge/github/githubClient';
import { butlerModule } from '$lib/state/butlerModule';
import { createGitHubApi } from '$lib/state/clientState.svelte';
import { Octokit } from '@octokit/rest';
import { configureStore, type ThunkDispatch, type UnknownAction } from '@reduxjs/toolkit';

/**
 * Mock for GitHub RTKQ.
 *
 * The `state` object is intentionally not declared using `$state`, that
 * would prevent tests from mutating state unless inside an `$effect`.
 *
 * @example
 * describe('some test', () => {
 * ...
 *   const { gitHubApi, octokit } = setupMockGitHubApi();
 *   const gh = new GitHub({
 *     gitHubApi,
 *     ...
 *   });
 *   const service = gh.prService;
 */

export function setupMockGitHubApi() {
	let state = {};
	let dispatch: ThunkDispatch<any, any, UnknownAction> | undefined = undefined;

	const backend = createBackend();
	const octokit = new Octokit();
	const gitHubClient = new GitHubClient({ client: octokit });
	gitHubClient.setRepo({ owner: 'test-owner', repo: 'test-repo' });
	const gitHubApi = createGitHubApi(
		butlerModule({ getDispatch: () => dispatch!, getState: () => () => state })
	);

	const store = configureStore({
		reducer: { github: gitHubApi.reducer },
		middleware: (getDefaultMiddleware) => {
			return getDefaultMiddleware({
				thunk: { extraArgument: { backend, gitHubClient } }
			}).concat(gitHubApi.middleware);
		}
	});

	store.subscribe(() => {
		state = store.getState();
	});
	dispatch = store.dispatch;

	/** Clears state and resets api object. */
	function resetGitHubMock() {
		state = {};
		dispatch?.(gitHubApi.util.resetApiState());
	}

	return {
		gitHubApi,
		gitHubClient,
		octokit,
		resetGitHubMock
	};
}
