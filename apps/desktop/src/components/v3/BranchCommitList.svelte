<script lang="ts">
	import type { Commit, UpstreamCommit } from '$lib/branches/v3';

	interface Props {
		commits: UpstreamCommit[] | Commit[];
	}

	const { commits }: Props = $props();
</script>

<div class="commits">
	{#each commits as commit, i (commit.id)}
		{@const first = i === 0}
		{@const last = i === commits.length - 1}
		<div class="commit" class:first class:last>
			<span>{commit.message}</span> <span class="text-clr3">{commit.id.substring(0, 7)}</span>
		</div>
	{/each}
</div>

<style lang="postcss">
	.commits {
		position: relative;
		display: flex;
		flex-direction: column;
		border-radius: 0 0 var(--radius-m) var(--radius-m);
	}

	.commit {
		position: relative;
		display: flex;
		gap: 12px;
		width: 100%;
		padding: 16px;
		background-color: var(--clr-bg-1);
		transition: background-color var(--transition-fast);

		&:focus {
			outline: none;
		}

		&:hover {
			& :global(.commit-actions-menu) {
				--show: true;
			}
		}

		&:not(.is-commit-open) {
			&:hover {
				background-color: var(--clr-bg-1-muted);

				& .commit__drag-icon {
					opacity: 1;
				}
			}
		}

		&.not-draggable {
			&:hover {
				& .commit__drag-icon {
					pointer-events: none;
					opacity: 0;
				}
			}
		}

		&:not(.no-border) {
			border-bottom: 1px solid var(--clr-border-2);
		}

		&.last {
			border-radius: 0 0 var(--radius-m) var(--radius-m);
			border-bottom: 0;
		}
	}
</style>
