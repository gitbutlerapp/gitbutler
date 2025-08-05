<script lang="ts">
	import { page } from '$app/state';
	import VibeControlPage from '$components/vibeControl/VibeControlPage.svelte';

	// TODO: Refactor so we don't need non-null assertion.
	const projectId = $derived(page.params.projectId!);
</script>

<VibeControlPage {projectId} />
