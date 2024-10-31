<script lang="ts">
	import type { CloudDiffSection } from '$lib/cloud/types';
	import type { Snippet } from 'svelte';

	interface Props {
		diffSection: CloudDiffSection;
		sectionControls: Snippet<[string]>;
	}

	const { diffSection, sectionControls }: Props = $props();
</script>

<div class="card">
	<div class="card__header">
		<p class="text-15 text-bold">{diffSection.newPath || diffSection.oldPath}</p>
		{@render sectionControls(diffSection.identifier)}
	</div>
	<div class="card__content no-overflow">
		<pre>{diffSection.diffPatch}</pre>
	</div>
</div>

<style lang="postcss">
	.no-overflow {
		overflow: hidden;
	}
</style>
