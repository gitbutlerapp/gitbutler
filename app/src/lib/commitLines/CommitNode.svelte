<script lang="ts">
	import type { CommitNode, Style } from '$lib/commitLines/types';

	interface Props {
		commitNode: CommitNode;
		style: Style;
	}

	const { commitNode, style }: Props = $props();
</script>

<div
	class="container"
	class:none={style === 'none'}
	class:remote={style === 'remote'}
	class:local={style === 'local'}
	class:local-and-remote={style === 'localAndRemote'}
	class:shadow={style === 'shadow'}
	class:integrated={style === 'integrated'}
>
	{#if commitNode.type === 'large' && commitNode.commit}
		<div class="large-node">
			<img
				class="avatar"
				alt="Gravatar for {commitNode.commit.author.email}"
				srcset="{commitNode.commit.author.gravatarUrl} 2x"
				width="100"
				height="100"
			/>
		</div>
	{:else}
		<div class="small-node"></div>
	{/if}
</div>

<style lang="postcss">
	.container {
		z-index: var(--z-lifted);

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
		width: 12px;
		height: 12px;

		border-radius: 6px;
	}
</style>
