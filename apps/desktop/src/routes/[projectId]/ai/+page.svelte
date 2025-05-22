<script lang="ts">
	import AIPage from '$components/ai2/AIPage.svelte';

	const { data } = $props();

	const { projectId } = $derived(data);
</script>

<AIPage {projectId} />
