import { goto } from '$app/navigation';
import { codegenPath } from '$lib/routes/routes.svelte';
import { UI_STATE } from '$lib/state/uiState.svelte';
import { inject } from '@gitbutler/core/context';

export function useGoToCodegenPage() {
	const uiState = inject(UI_STATE);

	function goToCodegenPage(projectId: string, stackId: string, branchName: string) {
		const projectState = $derived(uiState.project(projectId));

		projectState.selectedClaudeSession.set({
			stackId: stackId,
			head: branchName
		});
		goto(codegenPath(projectId));
	}

	return {
		goToCodegenPage
	};
}
