<script lang="ts">
	import Tooltip from '$lib/Tooltip.svelte';
	import { isDefined } from '$lib/utils/typeguards';
	import type { CellType, CommitNodeData } from '$lib/commitLinesStacking/types';

	interface Props {
		commitNode: CommitNodeData;
		type: CellType;
	}

	const { commitNode, type }: Props = $props();

	const isSquircle = $derived(['Remote', 'Upstream', 'Integrated', 'LocalShadow'].includes(type));

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
	class:remote={type === 'LocalRemote'}
	class:local={type === 'Local'}
	class:local-shadow={type === 'LocalShadow'}
	class:upstream={type === 'Upstream'}
	class:integrated={type === 'Integrated'}
>
	<Tooltip text={hoverText}>
		<div class="commit-node-dot" class:squircle={isSquircle}></div>
	</Tooltip>
	{#if type === 'LocalShadow'}
		<Tooltip text={hoverText}>
			<div class="commit-node-dot secondary"></div>
		</Tooltip>
	{/if}
</div>

<style lang="postcss">
	.container {
		position: relative;
		z-index: var(--z-ground);

		&.remote {
			--border-color: var(--clr-commit-remote);
		}

		&.local {
			--border-color: var(--clr-commit-local);
		}

		&.local-shadow {
			--border-color: var(--clr-commit-local);
		}

		&.local-shadow .secondary {
			--border-color: var(--clr-commit-remote);
		}

		&.upstream {
			--border-color: var(--clr-commit-upstream);
		}

		&.integrated {
			--border-color: var(--clr-commit-integrated);
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

			&.squircle {
				height: 0.625rem;
				width: 0.625rem;
				border-radius: 0.2rem;
				transform: rotate(45deg);
			}

			&.secondary {
				height: 0.7rem;
				width: 0.7rem;
				position: absolute;
				top: -0.025rem;
				left: -0.5rem;

				border-radius: 0.2rem;
				border: 0.175rem solid var(--clr-bg-2);
				transform: rotate(45deg);
			}
		}
	}
</style>
