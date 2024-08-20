<script lang="ts">
	import LineGroup from '$lib/commitLines/LineGroup.svelte';
	import type { LineGroupData } from '$lib/commitLines/types';

	interface Props {
		lineGroups: LineGroupData[];
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
