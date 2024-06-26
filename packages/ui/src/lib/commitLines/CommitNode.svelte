<script lang="ts">
	import { tooltip } from '$lib/utils/tooltip';
	import { isDefined } from '$lib/utils/typeguards';
	import type { CommitNode, Color } from '$lib/commitLines/types';

	interface Props {
		commitNode: CommitNode;
		color: Color;
	}

	const { commitNode, color }: Props = $props();

	const hoverText = $derived(
		[
			commitNode.commit?.author.name,
			commitNode.commit?.title,
			commitNode.commit?.id.substring(0, 7)
		]
			.filter(isDefined)
			.join('\n')
	);
</script>

<div
	class="container"
	class:none={color === 'none'}
	class:remote={color === 'remote'}
	class:local={color === 'local'}
	class:local-and-remote={color === 'localAndRemote'}
	class:shadow={color === 'shadow'}
	class:integrated={color === 'integrated'}
>
	{#if commitNode.type === 'large' && commitNode.commit}
		<div class="large-node">
			<img
				class="avatar"
				alt="Gravatar for {commitNode.commit.author.email}"
				srcset="{commitNode.commit.author.gravatarUrl} 2x"
				width="100"
				height="100"
				use:tooltip={hoverText}
			/>
		</div>
	{:else}
		<div class="small-node" use:tooltip={hoverText}></div>
	{/if}
</div>

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
	}

	.avatar {
		position: relative;
		width: 12px;
		height: 12px;

		border-radius: 6px;
	}
</style>
