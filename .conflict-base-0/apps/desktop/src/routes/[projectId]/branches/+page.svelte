<script lang="ts">
	import BranchesView from '$components/v3/BranchesView.svelte';
	import { page } from '$app/state';

	const projectId = $derived(page.params.projectId!);
</script>

<BranchesView {projectId} />

<style lang="postcss">
</style>
