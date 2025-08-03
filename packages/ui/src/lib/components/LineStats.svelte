<script lang="ts">
	import Tooltip from '$components/Tooltip.svelte';
	type Props = {
		linesAdded: number;
		linesRemoved: number;
	};

	const { linesAdded, linesRemoved }: Props = $props();
</script>

{#if linesAdded > 0 || linesRemoved > 0}
	<Tooltip text="Lines added/removed" delay={1200}>
		<div class="line-stats text-11 text-semibold">
			{#if linesAdded > 0}
				<span class="added">+{linesAdded}</span>
			{/if}
			{#if linesRemoved > 0}
				<span class="removed">-{linesRemoved}</span>
			{/if}
		</div>
	</Tooltip>
{/if}

<style lang="postcss">
	.line-stats {
		display: flex;
		align-items: center;
		width: fit-content;
		gap: 2px;

		& .added {
			color: var(--clr-theme-succ-element);
		}
		& .removed {
			color: var(--clr-theme-err-element);
		}
	}
</style>
