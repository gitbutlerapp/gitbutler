import { get as getHead, subscribe } from '$lib/api/git/heads';
import { asyncWritable, type Loadable, type WritableLoadable } from '@square/svelte-store';

export interface GitHeadStore extends Loadable<string> {
	subscribeStream(): () => void; // Consumer of store shall manage hsubscription
}

export function getHeadsStore(projectId: string): GitHeadStore {
	const store = asyncWritable(
		[],
		async () => {
			const head = await getHead({ projectId: projectId });
			return head.replace('refs/heads/', '');
		},
		async (data) => data,
		{ trackState: true }
	) as WritableLoadable<string>;
	const subscribeStream = () => {
		return subscribe({ projectId }, ({ head }) => {
			store.set(head.replace('refs/heads/', ''));
		});
	};
	return { ...store, subscribeStream };
}
