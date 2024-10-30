<script lang="ts">
	import DiffSection from '$lib/cloud/patches/DiffSection.svelte';
	import { CloudPatchService } from '$lib/cloud/patches/service';
	import { getContext } from '$lib/context';

	const cloudPatchService = getContext(CloudPatchService);
	const optionalPatch = cloudPatchService.patch;
</script>

{#if $optionalPatch.state === 'uninitialized'}
	<p>Loading...</p>
{:else if $optionalPatch.state === 'not-found'}
	<p>Failed to find patch</p>
{:else if $optionalPatch.state === 'found'}
	{@const patch = $optionalPatch.value}

	<h1 class="text-head-24 padding-bottom">Files:</h1>

	{#each patch.sections as section}
		{#if section.sectionType === 'diff'}
			<DiffSection diffSection={section} />
		{/if}
	{/each}
{/if}

<style lang="postcss">
	.padding-bottom {
		margin-bottom: 16px;
	}

	.two-by-two {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 8px;
	}
</style>
