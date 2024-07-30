<script lang="ts">
	import newProjectSvg from '$lib/assets/illustrations/new-project.svg?raw';
	import DecorativeSplitView from '$lib/components/DecorativeSplitView.svelte';
	import Welcome from '$lib/components/Welcome.svelte';
</script>

<DecorativeSplitView img={newProjectSvg} showLinks={false}>
	<Welcome />
</DecorativeSplitView>
