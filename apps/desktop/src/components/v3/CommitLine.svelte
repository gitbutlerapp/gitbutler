<script lang="ts">
	import { getColorFromBranchType, isLocalAndRemoteCommit } from '$components/v3/lib';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import { camelCaseToTitleCase } from '@gitbutler/ui/utils/string';
	import type { Commit, UpstreamCommit } from '$lib/branches/v3';

	interface Props {
		commit: Commit | UpstreamCommit;
		lastCommit?: boolean;
		lastBranch?: boolean;
		lineColor?: string;
	}

	const { commit, lastCommit, lastBranch, lineColor }: Props = $props();

	const color = $derived(
		lineColor ||
			(isLocalAndRemoteCommit(commit)
				? getColorFromBranchType(commit.state?.type ?? 'LocalOnly')
				: 'var(--clr-commit-upstream)')
	);
	const dotRhombus = $derived(
		isLocalAndRemoteCommit(commit) && commit.state.type === 'LocalAndRemote'
	);

	const tooltipText = $derived(
		!isLocalAndRemoteCommit(commit) ? 'Upstream' : camelCaseToTitleCase(commit.state.type)
	);
</script>

<div class="commit-lines" style:--commit-color={color}>
	<div class="top"></div>
	<Tooltip text={tooltipText}>
		<div class="middle" class:rhombus={dotRhombus}></div>
	</Tooltip>
	<div class="bottom" class:dashed={lastCommit && lastBranch}></div>
</div>

<style>
	.commit-lines {
		display: flex;
		flex-direction: column;
		align-items: center;
		margin: 0 16px;
		gap: 3px;
	}

	.top,
	.bottom {
		width: 2px;
		background-color: var(--commit-color);
	}

	.top {
		height: 14px;
	}

	.bottom {
		flex-grow: 1;
	}

	.middle {
		border-radius: 100%;
		width: 10px;
		height: 10px;
		background-color: var(--commit-color);

		&.rhombus {
			width: 10px;
			height: 10px;
			border-radius: 2px;
			transform: rotate(45deg) scale(0.86);
		}
	}
</style>
