<script lang="ts">
	import ScrollableContainer from "$components/shared/AppScrollableContainer.svelte";
	import { WORKTREE_SERVICE } from "$lib/worktree/worktreeService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Icon } from "@gitbutler/ui";

	type Props = {
		projectId: string;
	};

	let { projectId }: Props = $props();

	const worktreeService = inject(WORKTREE_SERVICE);
	const localIgnoredPathsQuery = $derived(worktreeService.localIgnoredPaths(projectId));
	const localIgnoredPaths = $derived(localIgnoredPathsQuery.response ?? []);

	let updatingPath = $state<string>();
	let collapsed = $state(false);

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
	<section class="local-ignored" class:collapsed>
		<div class="local-ignored__header">
			<button
				type="button"
				class="local-ignored__summary"
				aria-expanded={!collapsed}
				onclick={() => (collapsed = !collapsed)}
			>
				<Icon name="chevron-right" size={14} />
				<div>
					<h3 class="text-13 text-semibold">Locally ignored</h3>
					{#if !collapsed}
						<p class="text-12 text-body">
							Hidden only in this clone so Unity-generated churn stays out of your way.
						</p>
					{/if}
				</div>
			</button>
			<div class="local-ignored__meta">
				<span class="local-ignored__count">{localIgnoredPaths.length}</span>
			</div>
		</div>

		{#if !collapsed}
			<div class="local-ignored__scroller">
				<ScrollableContainer>
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
				</ScrollableContainer>
			</div>
		{/if}
	</section>
{/if}

<style lang="postcss">
	.local-ignored {
		display: flex;
		flex-shrink: 0;
		flex-direction: column;
		height: min(260px, 40vh);
		min-height: 116px;
		max-height: min(520px, 60vh);
		padding: 12px;
		overflow: hidden;
		gap: 12px;
		border-top: 1px solid var(--border-2);
		background: linear-gradient(
			180deg,
			var(--bg-1),
			color-mix(in srgb, var(--bg-2) 65%, transparent)
		);
		resize: vertical;

		&.collapsed {
			height: auto;
			min-height: 0;
			resize: none;
		}

		&.collapsed .local-ignored__summary :global(svg) {
			transform: rotate(0deg);
		}
	}

	.local-ignored__header {
		display: flex;
		flex-shrink: 0;
		align-items: flex-start;
		justify-content: space-between;
		gap: 12px;
	}

	.local-ignored__summary {
		display: flex;
		min-width: 0;
		padding: 0;
		gap: 8px;
		border: 0;
		background: transparent;
		color: inherit;
		text-align: left;

		& :global(svg) {
			flex-shrink: 0;
			margin-top: 1px;
			transform: rotate(90deg);
			color: var(--text-3);
			transition: transform var(--transition-fast);
		}
	}

	.local-ignored__summary h3,
	.local-ignored__summary p {
		margin: 0;
	}

	.local-ignored__summary p {
		margin-top: 4px;
		color: var(--text-3);
	}

	.local-ignored__meta {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		gap: 8px;
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

	.local-ignored__scroller {
		display: flex;
		flex: 1;
		min-height: 0;
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
