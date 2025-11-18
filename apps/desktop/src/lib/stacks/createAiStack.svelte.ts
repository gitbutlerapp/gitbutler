import { focusClaudeInput } from '$lib/codegen/focusClaudeInput';
import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
import { UI_STATE } from '$lib/state/uiState.svelte';
import { sleep } from '$lib/utils/sleep';
import { inject } from '@gitbutler/core/context';
import { untrack } from 'svelte';
import type { Reactive } from '@gitbutler/shared/storeUtils';

export function useCreateAiStack(projectId: Reactive<string>) {
	const uiState = inject(UI_STATE);
	const stackService = inject(STACK_SERVICE);

	async function createAiStack() {
		const pid = untrack(() => projectId.current);
		const stack = await stackService.newStackMutation({
			projectId: pid,
			branch: {
				name: undefined
			}
		});

		// Because the ui state is updated asyncly, we need to let some time
		// pass. This is far from a good solution to this problem.
		await sleep(50);
		uiState.project(pid).stackId.set(stack.id);
		if (!stack.id) return;

		const lane = uiState.lane(stack.id);
		lane.selection.set({ codegen: true, branchName: stack.heads[0]?.name, previewOpen: true });

		// I strongly dislike this sleep
		await sleep(50);
		focusClaudeInput(stack.id);
	}

	return { createAiStack };
}
