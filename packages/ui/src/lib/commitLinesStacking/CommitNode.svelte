<script lang="ts">
	import Tooltip from '$lib/Tooltip.svelte';
	import { isDefined } from '$lib/utils/typeguards';
	import type { CommitNodeData, Color, CellType } from '$lib/commitLinesStacking/types';

	interface Props {
		commitNode: CommitNodeData;
		color: CellType;
	}

	const { commitNode, color }: Props = $props();

	const hoverText = $derived(
		[
			commitNode.commit?.author?.name,
			commitNode.commit?.title,
			commitNode.commit?.id.substring(0, 7)
		]
			.filter(isDefined)
			.join('\n')
	);

	// $inspect('COMMIT_NODE', commitNode);
</script>

{#if commitNode.commit?.author}
	<div
		class="container"
		class:remote={color === 'Remote'}
		class:local={color === 'Local'}
		class:local-and-remote={color === 'LocalShadow'}
		class:integrated={color === 'Upstream'}
	>
		<!-- class:none={color === 'none'} -->
		<!-- class:shadow={color === 'Shadow'} -->
		{#if commitNode.commit}
			<div class="commit-node-dot"></div>
		{:else}
			<Tooltip text={hoverText}>
				<div class="small-node"></div>
			</Tooltip>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.container {
		z-index: var(--z-ground);

		&.none {
			--border-color: transparent;
		}

		&.remote {
			--border-color: var(--clr-commit-upstream);
		}

		&.local {
			--border-color: var(--clr-commit-local);
		}

		&.local-and-remote {
			--border-color: var(--clr-commit-remote);
		}

		&.shadow {
			--border-color: var(--clr-commit-shadow);
		}

		&.integrated {
			--border-color: var(--clr-commit-shadow);
		}

		& .small-node {
			height: 10px;
			width: 10px;

			margin-top: -5px;
			margin-bottom: -5px;
			margin-left: -6px;
			margin-right: -4px;
			background-color: var(--border-color);
			border-radius: 8px;
		}

		& .large-node {
			height: 16px;
			width: 16px;

			margin-top: -8px;
			margin-bottom: -8px;
			margin-left: -9px;
			margin-right: -7px;

			background-color: var(--border-color);
			border-radius: 8px;

			display: flex;
			align-items: center;
			justify-content: center;
		}

		.commit-node-dot {
			width: 8px;
			height: 8px;
			border-radius: 50%;
			background-color: red;
		}
	}
</style>
