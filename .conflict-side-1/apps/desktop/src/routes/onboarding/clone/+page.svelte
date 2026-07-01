<script lang="ts">
	import CloneForm from "$components/onboarding/CloneForm.svelte";
	import IllustrationSplitLayout from "$components/shared/IllustrationSplitLayout.svelte";
	import cloningRepoSvg from "$lib/assets/illustrations/cloning-repo.svg?raw";
</script>

<IllustrationSplitLayout img={cloningRepoSvg}>
	<CloneForm />
</IllustrationSplitLayout>
