<script lang="ts">
	import type { CloudDiffSection } from '$lib/cloud/types';

	interface Props {
		diffSection: CloudDiffSection;
	}

	const { diffSection }: Props = $props();
</script>

<div class="card">
	<p class="card__header text-15 text-bold">{diffSection.newPath || diffSection.oldPath}</p>
	<div class="card__content no-overflow">
		<pre>{diffSection.diffPatch}</pre>
	</div>
</div>

<style lang="postcss">
	.no-overflow {
		overflow: hidden;
	}
</style>
