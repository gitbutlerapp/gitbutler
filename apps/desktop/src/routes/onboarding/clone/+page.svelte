<script lang="ts">
	import cloningRepoSvg from '$lib/assets/illustrations/cloning-repo.svg?raw';
	import DecorativeSplitView from '$lib/components/DecorativeSplitView.svelte';
	import CloneForm from '$lib/onboarding/CloneForm.svelte';
</script>

<DecorativeSplitView img={cloningRepoSvg}>
	<CloneForm />
</DecorativeSplitView>
