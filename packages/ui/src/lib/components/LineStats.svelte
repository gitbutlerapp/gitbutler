<script lang="ts">
	import Tooltip from '$components/Tooltip.svelte';
	type Props = {
		linesAdded: number;
		linesRemoved: number;
		showSeparator?: boolean;
	};

	const { linesAdded, linesRemoved, showSeparator = true }: Props = $props();
</script>

{#if linesAdded > 0 || linesRemoved > 0}
	<div class="lines-stats-container text-11">
		{#if showSeparator}
			<span class="separator">/</span>
		{/if}
		<Tooltip text="Lines added/removed" delay={1200}>
			<div class="line-stats">
				{#if linesAdded > 0}
					<span class="added text-semibold">+{linesAdded}</span>
				{/if}
				{#if linesRemoved > 0}
					<span class="removed text-semibold">-{linesRemoved}</span>
				{/if}
			</div>
		</Tooltip>
	</div>
{/if}

<style lang="postcss">
	.lines-stats-container {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.separator {
		color: var(--clr-text-3);
	}

	.line-stats {
		display: flex;
		align-items: center;
		gap: 3px;

		& .added {
			color: var(--clr-theme-succ-element);
		}
		& .removed {
			color: var(--clr-theme-err-element);
		}
	}
</style>
