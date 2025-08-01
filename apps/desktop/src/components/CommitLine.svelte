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
	}

	const { commitStatus, diverged, tooltip, lastCommit, lastBranch }: Props = $props();

	const color = $derived(getColorFromCommitState(commitStatus, diverged));

	const rhombus = $derived(commitStatus === 'LocalAndRemote');
</script>

<div class="commit-lines" style:--commit-color={color}>
	<div class="top"></div>
	{#if diverged}
		<div class="local-shadow-commit-dot">
			<Tooltip text="Diverged">
				<svg class="shadow-dot" viewBox="0 0 10 10" xmlns="http://www.w3.org/2000/svg">
					<path
						d="M0.827119 6.41372C0.0460709 5.63267 0.0460709 4.36634 0.827119 3.58529L3.70602 0.706392C4.48707 -0.0746567 5.7534 -0.0746567 6.53445 0.706392L9.41335 3.58529C10.1944 4.36634 10.1944 5.63267 9.41335 6.41372L6.53445 9.29262C5.7534 10.0737 4.48707 10.0737 3.70602 9.29262L0.827119 6.41372Z"
					/>
				</svg>
			</Tooltip>
			<Tooltip text="Diverged">
				<svg class="local-dot" viewBox="0 0 11 10" xmlns="http://www.w3.org/2000/svg">
					<path
						fill-rule="evenodd"
						clip-rule="evenodd"
						d="M0.740712 8.93256C1.59096 9.60118 2.66337 10 3.82893 10H5.82893C8.59035 10 10.8289 7.76142 10.8289 5C10.8289 2.23858 8.59035 0 5.82893 0H3.82893C2.66237 0 1.58912 0.399504 0.738525 1.06916L1.84289 2.17353C3.40499 3.73562 3.40499 6.26828 1.84289 7.83038L0.740712 8.93256Z"
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
</div>

<style lang="postcss">
	.commit-lines {
		display: flex;
		flex: 0 0 auto;
		flex-direction: column;
		align-items: center;
		width: 42px;
		gap: 3px;
	}

	.top,
	.bottom {
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
		margin-left: -8px;

		.shadow-dot {
			width: 10px;
			height: 10px;
			fill: var(--clr-commit-shadow);
			margin-right: -1px;
		}

		.local-dot {
			width: 11px;
			height: 10px;
			fill: var(--clr-commit-local);
		}
	}
</style>
