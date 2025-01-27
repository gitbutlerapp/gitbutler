import { reduxApi } from '$lib/redux/api';
import { DesktopRedux } from '$lib/redux/store.svelte';
import { ReduxTag } from '$lib/redux/tags';
import type { Stack } from './stack';

export class StackService {
	private stacksApi = reduxApi.injectEndpoints({
		endpoints: (build) => ({
			get: build.query<Stack[], { projectId: string }>({
				query: ({ projectId }) => ({ command: 'stacks', params: { projectId } }),
				providesTags: [ReduxTag.Stacks]
			}),
			new: build.mutation<Stack, { projectId: string }>({
				query: ({ projectId }) => ({
					command: 'create_virtual_branch',
					params: { projectId, branch: {} }
				}),
				invalidatesTags: [ReduxTag.Stacks]
			})
		})
	});

	constructor(private state: DesktopRedux) {}

	getAll(projectId: string) {
		$effect(() => {
			const { unsubscribe } = this.state.dispatch(
				this.stacksApi.endpoints.get.initiate({ projectId })
			);
			return () => {
				unsubscribe();
			};
		});
		return this.stacksApi.endpoints.get.select({ projectId })(this.state.rootState$);
	}

	// eslint-disable-next-line @typescript-eslint/promise-function-async
	new(projectId: string) {
		return this.state.dispatch(this.stacksApi.endpoints.new.initiate({ projectId }));
	}
}
