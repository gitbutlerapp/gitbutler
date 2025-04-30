<script lang="ts">
	import { page } from '$app/state';
	import StackView from '$components/v3/StackView.svelte';

	const projectId = $derived(page.params.projectId!);
	const stackId = $derived(page.params.stackId!);
</script>

<StackView {projectId} {stackId} />
