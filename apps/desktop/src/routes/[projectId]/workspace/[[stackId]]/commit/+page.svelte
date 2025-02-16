<script lang="ts">
	import NewCommit from '$components/v3/NewCommit.svelte';
	import type { PageData } from './$types';
	import { page } from '$app/state';

	const { data }: { data: PageData } = $props();

	const projectId = $derived(data.projectId);
	const stackId = $derived(page.params.stackId!);
</script>

<NewCommit {projectId} {stackId} />
