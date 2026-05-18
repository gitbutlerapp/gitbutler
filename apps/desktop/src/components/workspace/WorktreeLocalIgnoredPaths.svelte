<script lang="ts">
	import { WORKTREE_SERVICE } from "$lib/worktree/worktreeService.svelte";
	import { inject } from "@gitbutler/core/context";

	type Props = {
		projectId: string;
	};

	let { projectId }: Props = $props();

	const worktreeService = inject(WORKTREE_SERVICE);
	const localIgnoredPathsQuery = $derived(worktreeService.localIgnoredPaths(projectId));
	const localIgnoredPaths = $derived(localIgnoredPathsQuery.response ?? []);

	let updatingPath = $state<string>();

	async function stopIgnoring(path: string) {
		updatingPath = path;
		try {
			await worktreeService.setLocalIgnoredPath(projectId, path, false);
		} finally {
			if (updatingPath === path) {
				updatingPath = undefined;
			}
		}
	}
</script>

{#if localIgnoredPaths.length > 0}
	<section class="local-ignored">
		<div class="local-ignored__header">
			<div>
				<h3 class="text-13 text-semibold">Locally ignored</h3>
				<p class="text-12 text-body">
					Hidden only in this clone so Unity-generated churn stays out of your way.
				</p>
			</div>
			<span class="local-ignored__count">{localIgnoredPaths.length}</span>
		</div>

		<ul class="local-ignored__list">
			{#each localIgnoredPaths as path (path)}
				<li class="local-ignored__item">
					<span class="local-ignored__path">{path}</span>
					<button
						type="button"
						class="local-ignored__button"
						aria-label={`Stop ignoring ${path}`}
						disabled={updatingPath === path}
						onclick={() => stopIgnoring(path)}
					>
						Stop ignoring
					</button>
				</li>
			{/each}
		</ul>
	</section>
{/if}

<style lang="postcss">
	.local-ignored {
		display: flex;
		flex-direction: column;
		padding: 12px;
		gap: 12px;
		border-top: 1px solid var(--border-2);
		background: linear-gradient(
			180deg,
			var(--bg-1),
			color-mix(in srgb, var(--bg-2) 65%, transparent)
		);
	}

	.local-ignored__header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 12px;
	}

	.local-ignored__header h3,
	.local-ignored__header p {
		margin: 0;
	}

	.local-ignored__header p {
		margin-top: 4px;
		color: var(--text-3);
	}

	.local-ignored__count {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-width: 24px;
		height: 24px;
		padding: 0 8px;
		border-radius: 999px;
		background-color: var(--bg-3);
		color: var(--text-2);
		font-weight: 600;
		font-size: 12px;
	}

	.local-ignored__list {
		display: flex;
		flex-direction: column;
		margin: 0;
		padding: 0;
		gap: 8px;
		list-style: none;
	}

	.local-ignored__item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 10px 12px;
		gap: 10px;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
		background-color: var(--bg-2);
	}

	.local-ignored__path {
		min-width: 0;
		overflow: hidden;
		color: var(--text-2);
		font-size: 12px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.local-ignored__button {
		flex-shrink: 0;
		padding: 6px 10px;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-s);
		background-color: var(--bg-1);
		color: var(--text-2);
		font-weight: 600;
		font-size: 12px;
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast),
			color var(--transition-fast);

		&:hover:not(:disabled) {
			border-color: var(--border-4);
			background-color: var(--bg-3);
			color: var(--text-1);
		}

		&:disabled {
			cursor: wait;
			opacity: 0.6;
		}
	}
</style>
