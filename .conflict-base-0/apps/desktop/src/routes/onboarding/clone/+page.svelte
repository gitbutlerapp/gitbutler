<script lang="ts">
	import CloneForm from '$components/CloneForm.svelte';
	import DecorativeSplitView from '$components/DecorativeSplitView.svelte';
	import cloningRepoSvg from '$lib/assets/illustrations/cloning-repo.svg?raw';
</script>

<DecorativeSplitView img={cloningRepoSvg}>
	<CloneForm />
</DecorativeSplitView>
