<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import CodegenPage from '$components/codegen/CodegenPage.svelte';
	import { codegenEnabled } from '$lib/config/uiFeatureFlags';
	import { workspacePath } from '$lib/routes/routes.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/core/context';

	// TODO: Refactor so we don't need non-null assertion.
	const projectId = $derived(page.params.projectId!);
	
	const uiState = inject(UI_STATE);
	const stackService = inject(STACK_SERVICE);
	const projectState = $derived(uiState.project(projectId));

	// Check for stackId in URL query parameters
	const urlStackId = $derived(page.url.searchParams.get('stackId'));
	const stacks = $derived(stackService.stacks(projectId));

	// Redirect users when feature is disabled
	$effect(() => {
		if (!$codegenEnabled) {
			goto(workspacePath(projectId));
		}
	});

	// Handle stackId URL parameter to auto-select branch
	$effect(() => {
		if (urlStackId && stacks.current.data) {
			// Find the stack with the matching ID
			const targetStack = stacks.current.data.find(s => s.id === urlStackId);
			if (targetStack && targetStack.heads.length > 0) {
				// Set the selected Claude session to the first head of this stack
				projectState.selectedClaudeSession.set({
					stackId: urlStackId,
					head: targetStack.heads[0].name
				});

				// Remove the stackId parameter from the URL to keep it clean
				const newUrl = new URL(page.url);
				newUrl.searchParams.delete('stackId');
				goto(newUrl.toString(), { replaceState: true });
			}
		}
	});
</script>

{#if $codegenEnabled}
	<CodegenPage {projectId} />
{:else}
	<!-- Show nothing while redirect is happening -->
{/if}
