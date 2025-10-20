<script lang="ts">
	import { page } from '$app/state';
	import CodegenPage from '$components/codegen/CodegenPage.svelte';

	// TODO: Refactor so we don't need non-null assertion.
	const projectId = $derived(page.params.projectId!);
</script>

<CodegenPage {projectId} />
