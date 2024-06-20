<script lang="ts">
	import Line from '$lib/commitLines/Line.svelte';
	import type { LineGroup } from '$lib/commitLines/types';

	interface Props {
		lineGroup: LineGroup;
	}

	const { lineGroup }: Props = $props();
</script>

<div class="line-group">
	{#each lineGroup.lines as line}
		<div class="line">
			<Line {line} />
		</div>
	{/each}
</div>

<style lang="postcss">
	.line-group {
		display: flex;
		height: 100%;
	}

	.line {
		height: 100%;
		width: 24px;
		margin-right: -2px;
	}
</style>
