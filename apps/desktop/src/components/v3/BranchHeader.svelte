<script lang="ts">
	import type { WorkspaceBranch } from '$lib/branches/v3';

	interface Props {
		branch: WorkspaceBranch;
		// branch: PatchSeries;
		isTopBranch: boolean;
		// lastPush: Date | undefined;
	}

	// const { branch, isTopBranch, lastPush }: Props = $props();
	const { branch, isTopBranch }: Props = $props();
</script>

<div>{branch.name}</div>

<style lang="postcss">
	.branch-header {
		position: relative;
		display: flex;
		align-items: center;
		flex-direction: column;

		&:not(:last-child) {
			border-bottom: 1px solid var(--clr-border-2);
		}

		&:hover,
		&:focus-within {
			& :global(.branch-actions-menu) {
				--show: true;
			}
		}
	}

	.branch-info {
		width: 100%;
		padding-right: 14px;
		display: flex;
		justify-content: flex-start;
		align-items: center;

		.remote-name {
			min-width: max-content;
			padding: 0 0 0 2px;
			color: var(--clr-scale-ntrl-60);
		}
	}

	.branch-info__name {
		display: flex;
		align-items: center;
		justify-content: flex-start;
		min-width: 0;
		flex-grow: 1;
	}

	.branch-info__content {
		overflow: hidden;
		flex: 1;
		width: 100%;
		display: flex;
		flex-direction: column;
		gap: 6px;
		padding: 14px 0;
		margin-left: -2px;
	}

	.branch-action {
		width: 100%;
		display: flex;
		justify-content: flex-start;
		align-items: stretch;

		.branch-action__body {
			width: 100%;
			padding: 0 14px 14px 0;
			display: flex;
			flex-direction: column;
			gap: 14px;
		}
	}

	.branch-action__line {
		min-width: 2px;
		margin: 0 22px 0 20px;
		background-color: var(--bg-color, var(--clr-border-3));
	}
</style>
