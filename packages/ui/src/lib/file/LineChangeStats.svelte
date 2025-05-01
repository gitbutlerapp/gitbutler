<script lang="ts">
	import Tooltip from '$lib/Tooltip.svelte';

	interface Props {
		added?: number;
		removed?: number;
	}

	const { added, removed }: Props = $props();
</script>

{#if added || removed}
	<Tooltip text="Lines added/removed" delay={1200}>
		<div class="file-line-stats__lines text-11 text-semibold">
			{#if added}
				<span class="file-line-stats__lines--added">+{added}</span>
			{/if}
			{#if removed}
				<span class="file-line-stats__lines--removed">-{removed}</span>
			{/if}
		</div>
	</Tooltip>
{/if}

<style lang="postcss">
	.file-line-stats__lines {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.file-line-stats__lines--added {
		color: var(--clr-theme-succ-element);
	}

	.file-line-stats__lines--removed {
		color: var(--clr-theme-err-element);
	}
</style>
