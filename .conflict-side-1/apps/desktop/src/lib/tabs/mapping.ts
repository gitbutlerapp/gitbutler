import type { Stack } from '$lib/stacks/stack';
import type { Tab } from './tab';

export function stacksToTabs(stacks: Stack[] | undefined): Tab[] {
	if (!stacks) {
		return [];
	}
	return stacks.map((stack) => {
		return {
			id: stack.id,
			name: stack.branchNames[0] || 'new branch',
			anchors: stack.branchNames.slice(1)
		};
	});
}
