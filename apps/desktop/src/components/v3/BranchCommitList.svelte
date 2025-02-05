<script lang="ts">
	import CommitRow from '$components/v3/CommitRow.svelte';
	import type { Commits } from '$lib/branches/v3';

	interface Props {
		commits: Commits;
	}

	const { commits }: Props = $props();

	const localAndRemoteCommits = $derived(commits.localAndRemote);
	const upstreamOnlyCommits = $derived(commits.upstreamOnly);
</script>

<div class="commit-list">
	{#each upstreamOnlyCommits as commit, i (commit.id)}
		{@const first = i === 0}
		{@const last = i === upstreamOnlyCommits.length - 1}
		<CommitRow {first} {last} {commit} />
	{/each}

	{#each localAndRemoteCommits as commit, i (commit.id)}
		{@const first = i === 0}
		{@const last = i === localAndRemoteCommits.length - 1}
		<CommitRow {first} {last} {commit} />
	{/each}
</div>

<style lang="postcss">
	.commit-list {
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
