import { reduxApi } from '$lib/redux/api';
import { DesktopRedux } from '$lib/redux/store.svelte';
import type { Stack } from './stack';

enum Tags {
	Stacks = 'Stacks'
}

export class StackService {
	private stacksApi = reduxApi.injectEndpoints({
		endpoints: (build) => ({
			get: build.query<Stack[], { projectId: string }>({
				query: ({ projectId }) => ({ command: 'stacks', params: { projectId } }),
				providesTags: [Tags.Stacks]
			}),
			new: build.mutation<Stack, { projectId: string }>({
				query: ({ projectId }) => ({
					command: 'create_virtual_branch',
					params: { projectId, branch: {} }
				}),
				invalidatesTags: [Tags.Stacks]
			})
		})
	});

	constructor(private state: DesktopRedux) {}

	poll(projectId: string) {
		$effect(() => {
			const { unsubscribe } = this.state.dispatch(
				this.stacksApi.endpoints.get.initiate({ projectId })
			);
			return () => {
				unsubscribe();
			};
		});
	}

	select(projectId: string) {
		return this.stacksApi.endpoints.get.select({ projectId })(this.state.rootState$);
	}

	// eslint-disable-next-line @typescript-eslint/promise-function-async
	new(projectId: string) {
		return this.state.dispatch(this.stacksApi.endpoints.new.initiate({ projectId }));
	}
}
