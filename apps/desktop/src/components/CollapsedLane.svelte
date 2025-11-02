<script lang="ts">
	import CollapseStackButton from '$components/CollapseStackButton.svelte';
	import { Icon } from '@gitbutler/ui';

	type Props = {
		stackId?: string;
		branchNames?: string[];
		onUnfold: () => void;
	};

	const { stackId: _stackId, branchNames, onUnfold }: Props = $props();
</script>

<div class="folded-lane">
	<div class="folded-lane-head">
		<CollapseStackButton isFolded onclick={onUnfold} />
		<Icon name="draggable-wide" />
	</div>

	<div class="text-14 text-semibold stack-names">
		{#if branchNames && branchNames.length > 0}
			{#each branchNames as branchName}
				<span class="branch-name">{branchName}</span>

				{#if branchName !== branchNames[branchNames.length - 1]}
					<Icon name="text-link" color="var(--clr-text-3)" rotate={90} />
				{/if}
			{/each}
		{:else}
			<span class="branch-name">Folded Stack</span>
		{/if}
	</div>
</div>

<style lang="postcss">
	.folded-lane {
		display: flex;
		flex-direction: column;
		justify-content: space-between;
		height: 100%;
		padding: 6px 8px 18px;
		border-right: 1px solid var(--clr-border-2);
		background: linear-gradient(
			90deg,
			var(--clr-bg-2) 0%,
			var(--clr-bg-2) 70%,
			var(--clr-bg-3) 100%
		);
		color: var(--clr-text-3);
	}

	.folded-lane-head {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 6px;
	}

	.stack-names {
		display: flex;
		align-items: center;
		overflow: hidden;
		gap: 10px;
		transform: rotate(180deg);
		text-orientation: mixed;
		writing-mode: vertical-lr;
	}

	.branch-name {
		overflow: hidden;
		color: var(--clr-text-1);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
