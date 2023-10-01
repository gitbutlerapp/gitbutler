import { getHead, subscribe } from '$lib/api/git/heads';
import { asyncWritable, type Loadable } from '@square/svelte-store';

export function getHeadsStore(projectId: string): Loadable<string> {
	return asyncWritable(
		[],
		async () => await getHead(projectId),
		undefined,
		{ trackState: true },
		(set) => {
			const unsubscribe = subscribe(projectId, set);
			return () => unsubscribe();
		}
	);
}
