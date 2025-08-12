<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import CodegenPage from '$components/codegen/CodegenPage.svelte';
	import { codegenEnabled } from '$lib/config/uiFeatureFlags';
	import { workspacePath } from '$lib/routes/routes.svelte';

	// TODO: Refactor so we don't need non-null assertion.
	const projectId = $derived(page.params.projectId!);

	// Redirect users when feature is disabled
	$effect(() => {
		if (!$codegenEnabled) {
			goto(workspacePath(projectId));
		}
	});
</script>

{#if $codegenEnabled}
	<CodegenPage {projectId} />
{:else}
	<!-- Show nothing while redirect is happening -->
{/if}
