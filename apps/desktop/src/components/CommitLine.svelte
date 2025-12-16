<script lang="ts">
	import { getColorFromCommitState } from '$components/lib';
	import { type CommitStatusType } from '$lib/commits/commit';

	import { Tooltip } from '@gitbutler/ui';

	interface Props {
		commitStatus: CommitStatusType;
		diverged: boolean;
		tooltip?: string;
		lastCommit?: boolean;
		lastBranch?: boolean;
		hasConflicts?: boolean;
		alignDot?: 'center' | 'start';
		hideDot?: boolean;
	}

	const {
		commitStatus,
		diverged,
		tooltip,
		lastCommit,
		lastBranch,
		alignDot = 'center',
		hasConflicts,
		hideDot = false
	}: Props = $props();

	const color = $derived(getColorFromCommitState(commitStatus, diverged));

	const rhombus = $derived(commitStatus === 'LocalAndRemote');

	function getCommitColor() {
		if (hasConflicts) {
			return 'var(--clr-theme-danger-element)';
		}
		return color;
	}
</script>

<div
	class="commit-lines align-{alignDot}"
	style:--commit-color={getCommitColor()}
	style:--commit-local-color={hasConflicts
		? 'var(--clr-theme-danger-element)'
		: 'var(--clr-commit-local)'}
>
	{#if hideDot}
		<div class="single-line" class:dashed={lastCommit && lastBranch}></div>
	{:else}
		<div class="top"></div>
		{#if diverged}
			<div class="local-shadow-commit-dot">
				<Tooltip text="Diverged">
					<svg class="shadow-dot" viewBox="0 0 10 10" xmlns="http://www.w3.org/2000/svg">
						<path
							d="M0.827119 6.41372C0.0460709 5.63267 0.0460709 4.36634 0.827119 3.58529L3.70602 0.706392C4.48707 -0.0746567 5.7534 -0.0746567 6.53445 0.706392L9.41335 3.58529C10.1944 4.36634 10.1944 5.63267 9.41335 6.41372L6.53445 9.29262C5.7534 10.0737 4.48707 10.0737 3.70602 9.29262L0.827119 6.41372Z"
						/>
					</svg>
					<svg class="local-dot" viewBox="0 0 10 9" xmlns="http://www.w3.org/2000/svg">
						<path
							d="M2.88623 2.29395C4.05781 3.46548 5.95783 3.46551 7.12939 2.29395L8.73975 0.682617C9.52337 1.56536 10.0005 2.72678 10.0005 4C10.0005 6.76126 7.76169 8.99974 5.00049 9C2.23906 9 -0.000488281 6.76142 -0.000488281 4C-0.000488281 2.72278 0.479074 1.55758 1.26709 0.673828L2.88623 2.29395Z"
						/>
					</svg>
				</Tooltip>
			</div>
		{:else}
			<Tooltip text={tooltip}>
				<div class="middle" class:rhombus></div>
			</Tooltip>
		{/if}
		<div class="bottom" class:dashed={lastCommit && lastBranch}></div>
	{/if}
</div>

<style lang="postcss">
	.commit-lines {
		display: flex;
		flex: 0 0 auto;
		flex-direction: column;
		align-items: center;
		width: 42px;
		gap: 3px;

		&.align-start {
			.top {
				flex: 0 0 13px;
			}
		}

		&.align-center {
			.top {
				flex: 1;
			}
		}
	}

	.top,
	.bottom,
	.single-line {
		flex: 1;
		width: 2px;
		background-color: var(--commit-color);
	}

	.middle {
		width: 10px;
		height: 10px;
		border-radius: 100%;
		background-color: var(--commit-color);

		&.rhombus {
			width: 10px;
			height: 10px;
			transform: rotate(45deg) scale(0.86);
			border-radius: 2px;
		}
	}

	.dashed {
		background: linear-gradient(to bottom, var(--commit-color) 50%, transparent 50%);
		background-size: 4px 4px;
	}

	.local-shadow-commit-dot {
		box-sizing: border-box;
		display: flex;
		flex-direction: column;
		align-items: center;

		.shadow-dot {
			width: 10px;
			height: 10px;
			transform: scale(1.1);
			fill: var(--clr-commit-shadow);
		}

		.local-dot {
			width: 10px;
			height: 9px;
			margin-top: -1px;
			fill: var(--commit-local-color);
		}
	}
</style>
