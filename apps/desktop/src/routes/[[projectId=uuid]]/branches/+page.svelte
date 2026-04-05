<script lang="ts">
	import BranchesView from "$components/views/BranchesView.svelte";
	import type { PageData } from "./$types";

	const { data }: { data: PageData } = $props();
	const projectId = $derived(data.projectId);
</script>

<BranchesView {projectId} />
