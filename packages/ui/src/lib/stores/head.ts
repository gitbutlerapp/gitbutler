import { getHead, subscribeToHead } from '$lib/api/git/heads';
import { asyncWritable, type Loadable } from '@square/svelte-store';

export function getHeadsStore(projectId: string): Loadable<string> {
	return asyncWritable(
		[],
		async () => await getHead(projectId),
		undefined,
		{ trackState: true },
		(set) => {
			const unsubscribe = subscribeToHead(projectId, set);
			return () => unsubscribe();
		}
	);
}
