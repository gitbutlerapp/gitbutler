import { Tauri } from '$lib/backend/tauri';
import { GitHubClient } from '$lib/forge/github/githubClient';
import { butlerModule } from '$lib/state/butlerModule';
import { createGitHubApi } from '$lib/state/clientState.svelte';
import { Octokit } from '@octokit/rest';
import { configureStore, type ThunkDispatch, type UnknownAction } from '@reduxjs/toolkit';

export function setupMockGitHubApi() {
	let state = $state({});
	let dispatch: ThunkDispatch<any, any, UnknownAction> | undefined = $state(undefined);

	const tauri = new Tauri();
	const octokit = new Octokit();
	const gitHubClient = new GitHubClient({ client: octokit });
	const gitHubApi = createGitHubApi(
		butlerModule({ getDispatch: () => dispatch!, getState: () => () => state })
	);

	const store = configureStore({
		reducer: { github: gitHubApi.reducer },
		middleware: (getDefaultMiddleware) => {
			return getDefaultMiddleware({
				thunk: { extraArgument: { tauri, gitHubClient } }
			}).concat(gitHubApi.middleware);
		}
	});

	store.subscribe(() => {
		state = store.getState();
	});
	dispatch = store.dispatch;

	return { gitHubApi, octokit };
}
