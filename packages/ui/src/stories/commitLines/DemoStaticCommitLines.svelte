<script lang="ts">
	import LineGroup from '$lib/CommitLines/LineGroup.svelte';
	import type { LineGroup as ILineGroup } from '$lib/CommitLines/types.ts';

	interface Props {
		lineGroups: ILineGroup[];
	}

	const { lineGroups }: Props = $props();
</script>

<div class="column">
	{#each lineGroups as lineGroup}
		<div class="group">
			<LineGroup {lineGroup} />
		</div>
	{/each}
</div>

<style lang="postcss">
	.group {
		height: 68px;
	}
</style>
