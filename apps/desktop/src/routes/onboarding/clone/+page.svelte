<script lang="ts">
	import CloneForm from './CloneForm.svelte';
	import DecorativeSplitView from '$components/shared/DecorativeSplitView.svelte';
	import cloningRepoSvg from '$lib/assets/illustrations/cloning-repo.svg?raw';
</script>

<DecorativeSplitView img={cloningRepoSvg}>
	<CloneForm />
</DecorativeSplitView>
