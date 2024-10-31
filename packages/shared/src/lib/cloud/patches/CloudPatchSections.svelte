<script lang="ts">
	import DiffSection from '$lib/cloud/patches/DiffSection.svelte';
	import { PatchSectionsService } from '$lib/cloud/patches/sections';
	import { CloudPatchService } from '$lib/cloud/patches/service';
	import { getContext } from '$lib/context';
	import Button from '@gitbutler/ui/Button.svelte';

	const cloudPatchService = getContext(CloudPatchService);
	const patchSectionsService = getContext(PatchSectionsService);
	const optionalPatch = cloudPatchService.patch;
</script>

{#snippet sectionControls(identifier: string)}
	<div>
		<Button onclick={() => patchSectionsService.moveSectionUp(identifier)}>Move Up</Button>
		<Button onclick={() => patchSectionsService.moveSectionDown(identifier)}>Move Down</Button>
	</div>
{/snippet}

{#if $optionalPatch.state === 'uninitialized'}
	<p>Loading...</p>
{:else if $optionalPatch.state === 'not-found'}
	<p>Failed to find patch</p>
{:else if $optionalPatch.state === 'found'}
	{@const patch = $optionalPatch.value}

	<h1 class="text-head-24 padding-bottom">Files:</h1>

	{#each patch.sections as section}
		{#if section.sectionType === 'diff'}
			<DiffSection diffSection={section} {sectionControls} />
		{/if}
	{/each}
{/if}

<style lang="postcss">
	.padding-bottom {
		margin-bottom: 16px;
	}
</style>
