<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';

	interface Props {
		series: string[];
	}

	const { series }: Props = $props();

	console.log(series);
</script>

<div class="stack-meta">
	<div class="stack-branches">
		<div class="stack-first-branch">
			<div class="stack-icon">
				<Icon name={series.length > 1 ? 'chain-link' : 'branch-small'} />
			</div>

			<div class="series-name text-12 text-semibold">
				<span class="truncate">{series[0]}</span>
			</div>
		</div>
	</div>
</div>

<style lang="postcss">
	.stack-meta {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 12px;
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		border-top: none;
	}

	.stack-branches {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
	}

	.stack-first-branch {
		display: flex;
		align-items: center;
		gap: 4px;
		width: 100%;
	}

	.stack-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		width: 20px;
		height: 22px;
		color: var(--clr-text-2);
		background-color: var(--clr-theme-ntrl-soft);
		border-radius: var(--radius-m);
	}

	.series-name {
		display: flex;
		align-items: center;
		color: var(--clr-text-2);
		height: 22px;
		padding: 2px 6px;
		background-color: var(--clr-theme-ntrl-soft);
		border-radius: var(--radius-m);
		width: 100%;
		overflow: hidden;
		max-width: fit-content;
	}
</style>
