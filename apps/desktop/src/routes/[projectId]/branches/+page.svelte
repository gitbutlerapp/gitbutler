<script lang="ts">
	import { page } from "$app/state";
	import BranchesView from "$components/BranchesView.svelte";

	const projectId = $derived(page.params.projectId!);
</script>

<BranchesView {projectId} />
