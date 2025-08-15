<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import CodegenPage from '$components/codegen/CodegenPage.svelte';
	import { UI_FEATURE_FLAGS_SERVICE } from '$lib/config/uiFeatureFlags';
	import { workspacePath } from '$lib/routes/routes.svelte';
	import { inject } from '@gitbutler/shared/context';

	// TODO: Refactor so we don't need non-null assertion.
	const projectId = $derived(page.params.projectId!);
	const uiFlagsService = inject(UI_FEATURE_FLAGS_SERVICE);

	// Redirect users when feature is disabled
	$effect(() => {
		if (!$uiFlagsService.codegenEnabled) {
			goto(workspacePath(projectId));
		}
	});
</script>

{#if $uiFlagsService.codegenEnabled}
	<CodegenPage {projectId} />
{:else}
	<!-- Show nothing while redirect is happening -->
{/if}
