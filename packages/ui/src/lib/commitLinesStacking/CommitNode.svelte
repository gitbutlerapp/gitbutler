<script lang="ts">
	import Tooltip from '$lib/Tooltip.svelte';
	import { isDefined } from '$lib/utils/typeguards';
	import type { CommitNodeData, CellType } from '$lib/commitLinesStacking/types';

	interface Props {
		commitNode: CommitNodeData;
		type: CellType;
	}

	const { commitNode, type }: Props = $props();

	const hoverText = $derived(
		[
			commitNode.commit?.author?.name,
			commitNode.commit?.title,
			commitNode.commit?.id.substring(0, 7)
		]
			.filter(isDefined)
			.join('\n')
	);
</script>

<div
	class="container"
	class:remote={type === 'Remote'}
	class:local={type === 'Local'}
	class:local-and-remote={type === 'LocalShadow'}
	class:upstream={type === 'Upstream'}
	class:integrated={type === 'Integrated'}
>
	{#if commitNode.commit}
		<div class="commit-node-dot"></div>
	{:else}
		<Tooltip text={hoverText}>
			<div class="small-node"></div>
		</Tooltip>
	{/if}
</div>

<style lang="postcss">
	.container {
		z-index: var(--z-ground);

		&.remote {
			--border-color: var(--clr-commit-remote);
		}

		&.local {
			--border-color: var(--clr-commit-local);
		}

		&.local-and-remote {
			--border-color: var(--clr-commit-shadow);
		}

		&.upstream {
			--border-color: var(--clr-commit-upstream);
		}

		&.integrated {
			--border-color: var(--clr-commit-integrated);
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

		.commit-node-dot {
			height: 10px;
			width: 10px;
			margin-right: -4px;

			display: flex;
			align-items: center;
			justify-content: center;

			border-radius: 50%;
			background-color: var(--border-color);
		}
	}
</style>
