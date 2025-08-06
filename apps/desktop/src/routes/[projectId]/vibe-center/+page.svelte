<script lang="ts">
	import { page } from '$app/state';
	import VibeCenterPage from '$components/vibeCenter/VibeCenterPage.svelte';

	// TODO: Refactor so we don't need non-null assertion.
	const projectId = $derived(page.params.projectId!);
</script>

<VibeCenterPage {projectId} />
