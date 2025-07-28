<script lang="ts">
	import BranchesView from './View.svelte';
	import { page } from '$app/state';

	const projectId = $derived(page.params.projectId!);
</script>

<BranchesView {projectId} />
