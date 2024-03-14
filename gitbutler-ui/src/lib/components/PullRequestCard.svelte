<script lang="ts">
	import IconButton from './IconButton.svelte';
	import MergeButton from './MergeButton.svelte';
	import Tag, { type TagColor } from './Tag.svelte';
	import ViewPrContextMenu from '$lib/components/ViewPrContextMenu.svelte';
	import { sleep } from '$lib/utils/sleep';
	import * as toasts from '$lib/utils/toasts';
	import { openExternalUrl } from '$lib/utils/url';
	import { onDestroy } from 'svelte';
	import type { BranchService } from '$lib/branches/service';
	import type { GitHubService } from '$lib/github/service';
	import type { ChecksStatus, DetailedPullRequest } from '$lib/github/types';
	import type { BaseBranchService } from '$lib/vbranches/branchStoresCache';
	import type { Branch } from '$lib/vbranches/types';
	import type iconsJson from '../icons/icons.json';

	export let isLaneCollapsed: boolean;
	export let branchService: BranchService;
	export let branch: Branch;
	export let githubService: GitHubService;
	export let projectId: string;
	export let isUnapplied = false;
	export let baseBranchService: BaseBranchService;

	let isMerging = false;
	let isFetching = false;
	let isFetchingDetails = false;
	let checksStatus: ChecksStatus | null | undefined = undefined;
	let detailedPr: DetailedPullRequest | undefined;

	$: pr$ = githubService.getPr$(branch.upstreamName);
	$: if (branch && pr$) {
		isFetchingDetails = true;
		sleep(1000).then(() => {
			updateDetailedPullRequest($pr$?.targetBranch, false);
			fetchChecks();
		});
	}

	async function updateDetailedPullRequest(targetBranch: string | undefined, skipCache: boolean) {
		isFetchingDetails = true;
		try {
			detailedPr = await githubService.getDetailedPullRequest(targetBranch, skipCache);
		} catch {
			toasts.error('Failed to fetch PR details');
		} finally {
			isFetchingDetails = false;
		}
	}

	function updateContextMenu(copyablePrUrl: string) {
		if (popupMenu) popupMenu.$destroy();
		return new ViewPrContextMenu({
			target: document.body,
			props: { prUrl: copyablePrUrl }
		});
	}

	async function fetchChecks() {
		isFetching = true;
		try {
			checksStatus = await githubService.checks($pr$?.targetBranch);
		} catch (e: any) {
			if (!e.message.includes('No commit found')) {
				toasts.error('Failed to fetch PR status');
				console.error(e);
			}
			checksStatus = undefined;
		} finally {
			isFetching = false;
		}

		if (checksStatus) scheduleNextPrFetch();
	}

	function scheduleNextPrFetch() {
		if (checksStatus?.error) return 'error';
		if (!checksStatus || checksStatus.completed) {
			return;
		}
		const startedAt = checksStatus.startedAt;
		if (!startedAt) return;
		const secondsAgo = (new Date().getTime() - startedAt.getTime()) / 1000;
		let timeUntilUdate: number | undefined = undefined;
		if (secondsAgo < 600) {
			timeUntilUdate = 30;
		} else if (secondsAgo < 1200) {
			timeUntilUdate = 60;
		} else if (secondsAgo < 3600) {
			timeUntilUdate = 120;
		}
		if (!timeUntilUdate) {
			// Stop polling for status.
			return;
		}
		setTimeout(() => fetchChecks(), timeUntilUdate * 1000);
	}

	$: checksIcon = getChecksIcon(checksStatus, isFetching);
	$: checksColor = getChecksColor(checksStatus);
	$: checksText = getChecksText(checksStatus);
	$: statusIcon = getStatusIcon();
	$: statusColor = getStatusColor();
	$: statusLabel = getPrStatusLabel();

	// TODO: Refactor away the code duplication in the following functions
	function getChecksColor(status: ChecksStatus): TagColor | undefined {
		if (!status) return;
		if (!status.hasChecks) return 'ghost';
		if (status.error) return 'error';
		if (status.completed) {
			return status.success ? 'success' : 'error';
		}
		return 'warning';
	}

	function getChecksIcon(
		status: ChecksStatus,
		fetching: boolean
	): keyof typeof iconsJson | undefined {
		if (status === null) return;
		if (fetching || !status) return 'spinner';
		if (!status.hasChecks) return;
		if (status.error) return 'error-small';
		if (status.completed) {
			return status.success ? 'success-small' : 'error-small';
		}

		return 'spinner';
	}

	function getChecksText(status: ChecksStatus | undefined | null): string | undefined {
		if (!status) return 'Checks';
		if (!status.hasChecks) return 'No checks';
		if (status.error) return 'error';
		if (status.completed) {
			return status.success ? 'Checks passed' : 'Checks failed';
		}
		// Checking this second to last let's us keep the previous tag color unless
		// checks are currently running.
		if (isFetching) return 'Checks';
		return 'Checks are running';
	}

	function getPrStatusLabel(): string | undefined {
		if ($pr$?.mergedAt) return 'Merged';
		if ($pr$?.closedAt) return 'Closed';
		return 'Open';
	}

	function getStatusIcon(): keyof typeof iconsJson | undefined {
		if ($pr$?.mergedAt) return 'merged-pr-small';
		if ($pr$?.closedAt) return 'closed-pr-small';
		return 'pr-small';
	}

	function getStatusColor(): TagColor {
		if ($pr$?.mergedAt) return 'purple';
		if ($pr$?.closedAt) return 'error';
		return 'success';
	}

	$: popupMenu = updateContextMenu($pr$?.htmlUrl || '');

	onDestroy(() => {
		if (popupMenu) {
			popupMenu.$destroy();
		}
	});
</script>

{#if $pr$?.htmlUrl}
	{@const pr = $pr$}
	<div class="card pr-card">
		<div class="floating-button">
			<IconButton
				icon="update-small"
				loading={isFetchingDetails}
				on:click={async () => {
					updateDetailedPullRequest(pr?.targetBranch, true);
					fetchChecks();
				}}
			/>
		</div>
		<div class="pr-title text-base-13 font-semibold">
			<span style="color: var(--clr-theme-scale-ntrl-50)">PR #{pr.number}:</span>
			{pr.title}
		</div>
		<div class="pr-tags">
			<Tag
				icon={statusIcon}
				color={statusColor}
				filled={statusLabel !== 'Open'}
				verticalOrientation={isLaneCollapsed}
			>
				{statusLabel}
			</Tag>
			{#if branch.upstream && checksIcon}
				<Tag
					icon={checksIcon}
					color={checksColor}
					filled={checksIcon == 'success-small'}
					clickable
					verticalOrientation={isLaneCollapsed}
					on:mousedown={fetchChecks}
					help="Checks status"
				>
					{checksText}
				</Tag>
			{:else}
				<Tag
					color="light"
					verticalOrientation={isLaneCollapsed}
					help="There are no checks for this pull request"
				>
					No checks
				</Tag>
			{/if}
			<Tag
				icon="open-link"
				color="ghost"
				clickable
				border
				shrinkable
				verticalOrientation={isLaneCollapsed}
				on:mousedown={(e) => {
					const url = pr?.htmlUrl;
					if (url) openExternalUrl(url);
					e.preventDefault();
					e.stopPropagation();
				}}
				on:contextmenu={(e) => {
					e.preventDefault();
					popupMenu.openByMouse(e, undefined);
				}}
			>
				Open in browser
			</Tag>
		</div>

		<!--
        We can't show the merge button until we've waited for checks

        We use a octokit.checks.listForRef to find checks running for a PR, but right after
        creation this request succeeds but returns an empty array. So we need a better way
        determining "no checks will run for this PR" such that we can show the merge button
        immediately.
        -->
		{#if pr}
			<div class="pr-actions">
				<MergeButton
					{projectId}
					wide
					disabled={isFetching || isUnapplied || !pr || detailedPr?.mergeableState == 'blocked'}
					loading={isMerging}
					help="Merge pull request and refresh"
					on:click={async (e) => {
						if (!pr) return;
						isMerging = true;
						const method = e.detail.method;
						try {
							await githubService.merge(pr.number, method);
						} catch {
							// TODO: Should we show the error from GitHub?
							toasts.error('Failed to merge pull request');
						} finally {
							isMerging = false;
							await fetchChecks();
							await branchService.reloadVirtualBranches();
							baseBranchService.reload();
						}
					}}
				/>
			</div>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.pr-card {
		position: relative;
		padding: var(--space-14);
	}

	.pr-title {
		color: var(--clr-theme-scale-ntrl-0);
		margin-bottom: var(--space-12);
		user-select: text;
		cursor: text;
	}

	.pr-tags {
		display: flex;
		gap: var(--space-4);
	}

	.pr-actions {
		margin-top: var(--space-14);
		display: flex;
	}

	.floating-button {
		position: absolute;
		right: var(--space-6);
		top: var(--space-6);
	}
</style>
