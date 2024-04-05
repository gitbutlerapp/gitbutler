<script lang="ts">
	import IconButton from './IconButton.svelte';
	import MergeButton from './MergeButton.svelte';
	import Tag, { type TagStyle } from './Tag.svelte';
	import { Project } from '$lib/backend/projects';
	import { BranchService } from '$lib/branches/service';
	import ViewPrContextMenu from '$lib/components/ViewPrContextMenu.svelte';
	import { GitHubService } from '$lib/github/service';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { createTimeAgoStore } from '$lib/utils/timeAgo';
	import * as toasts from '$lib/utils/toasts';
	import { openExternalUrl } from '$lib/utils/url';
	import { BaseBranchService } from '$lib/vbranches/baseBranch';
	import { Branch } from '$lib/vbranches/types';
	import { onDestroy } from 'svelte';
	import type { ChecksStatus, DetailedPullRequest } from '$lib/github/types';
	import type iconsJson from '../icons/icons.json';
	import type { Readable } from 'svelte/store';

	export let isLaneCollapsed: boolean;

	const branch = getContextStore(Branch);
	const branchService = getContext(BranchService);
	const baseBranchService = getContext(BaseBranchService);
	const githubService = getContext(GitHubService);
	const project = getContext(Project);

	let isMerging = false;
	let isFetchingChecks = false;
	let isFetchingDetails = false;
	let checksError: string | undefined;
	let detailsError: string | undefined;
	let detailedPr: DetailedPullRequest | undefined;
	let mergeableState: string | undefined;
	let checksStatus: ChecksStatus | null | undefined = undefined;
	let lastDetailsFetch: Readable<string> | undefined;
	let lastChecksFetch: Readable<string> | undefined;

	$: pr$ = githubService.getPr$($branch.upstreamName);
	$: if ($branch && $pr$) updateDetailsAndChecks();

	$: checksIcon = getChecksIcon(checksStatus, isFetchingChecks);
	$: checksColor = getChecksColor(checksStatus);
	$: checksText = getChecksText(checksStatus);
	$: statusIcon = getStatusIcon(detailedPr);
	$: statusColor = getStatusColor(detailedPr);
	$: statusLabel = getPrStatusLabel(detailedPr);

	async function updateDetailsAndChecks() {
		if (!isFetchingChecks) fetchChecks();
		if (!isFetchingDetails) updateDetailedPullRequest($pr$?.targetBranch, false);
	}

	async function updateDetailedPullRequest(targetBranch: string | undefined, skipCache: boolean) {
		detailsError = undefined;
		isFetchingDetails = true;
		try {
			detailedPr = await githubService.getDetailedPr(targetBranch, skipCache);
			mergeableState = detailedPr?.mergeableState;
			lastDetailsFetch = createTimeAgoStore(new Date(), true);
		} catch (err: any) {
			detailsError = err.message;
			toasts.error('Failed to fetch PR details');
			console.error(err);
		} finally {
			isFetchingDetails = false;
		}
	}

	async function fetchChecks() {
		checksError = undefined;
		isFetchingChecks = true;
		try {
			checksStatus = await githubService.checks($pr$?.targetBranch);
			lastChecksFetch = createTimeAgoStore(new Date(), true);
		} catch (e: any) {
			console.error(e);
			checksError = e.message;
			checksStatus = { error: 'could not load checks' };
			if (!e.message.includes('No commit found')) {
				toasts.error('Failed to fetch checks');
			}
		} finally {
			isFetchingChecks = false;
		}

		if (checksStatus) scheduleNextUpdate();
	}

	function scheduleNextUpdate() {
		if (checksStatus?.error) return;
		if (!checksStatus || checksStatus.completed) return;

		const startedAt = checksStatus.startedAt;
		if (!startedAt) return;

		const secondsAgo = (new Date().getTime() - startedAt.getTime()) / 1000;
		let timeUntilUdate: number | undefined = undefined;

		if (secondsAgo < 60) {
			timeUntilUdate = 10;
		} else if (secondsAgo < 600) {
			timeUntilUdate = 30;
		} else if (secondsAgo < 1200) {
			timeUntilUdate = 60;
		} else if (secondsAgo < 3600) {
			timeUntilUdate = 120;
		} else if (secondsAgo < 7200) {
			// Stop polling after 2h
			timeUntilUdate = undefined;
		}
		if (!timeUntilUdate) {
			return;
		}
		setTimeout(() => updateDetailsAndChecks(), timeUntilUdate * 1000);
	}

	// TODO: Refactor away the code duplication in the following functions
	function getChecksColor(status: ChecksStatus): TagStyle | undefined {
		if (checksError || detailsError) return 'error';
		if (!status) return 'neutral';
		if (!status.hasChecks) return 'neutral';
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
		if (checksError || detailsError) return 'warning-small';
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
		if (checksError || detailsError) return 'Failed to load';
		if (!status) return 'Checks';
		if (!status.hasChecks) return 'No checks';
		if (status.error) return 'error';
		if (status.completed) {
			return status.success ? 'Checks passed' : 'Checks failed';
		}
		// Checking this second to last let's us keep the previous tag color unless
		// checks are currently running.
		if (isFetchingChecks) return 'Checks';
		return 'Checks are running';
	}

	function getPrStatusLabel(pr: DetailedPullRequest | undefined): string {
		if (pr?.mergedAt) return 'Merged';
		if (pr?.closedAt) return 'Closed';
		if (pr?.draft) return 'Draft';
		return 'Open';
	}

	function getStatusIcon(pr: DetailedPullRequest | undefined): keyof typeof iconsJson | undefined {
		if (pr?.mergedAt) return 'merged-pr-small';
		if (pr?.closedAt) return 'closed-pr-small';
		if (pr?.closedAt) return 'draft-pr-small';
		return 'pr-small';
	}

	function getStatusColor(pr: DetailedPullRequest | undefined): TagStyle {
		if (pr?.mergedAt) return 'purple';
		if (pr?.closedAt) return 'error';
		if (pr?.draft) return 'neutral';
		return 'success';
	}

	function updateContextMenu(copyablePrUrl: string) {
		if (popupMenu) popupMenu.$destroy();
		return new ViewPrContextMenu({
			target: document.body,
			props: { prUrl: copyablePrUrl }
		});
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
				size="m"
				loading={isFetchingDetails}
				help={$lastDetailsFetch ? 'Updated ' + $lastDetailsFetch : ''}
				on:click={async () => {
					await updateDetailsAndChecks();
				}}
			/>
		</div>
		<div class="pr-title text-base-13 font-semibold">
			<span style="color: var(--clr-scale-ntrl-50)">PR #{pr.number}:</span>
			{pr.title}
		</div>
		<div class="pr-tags">
			<Tag
				icon={statusIcon}
				style={statusColor}
				kind={statusLabel !== 'Open' ? 'solid' : 'soft'}
				verticalOrientation={isLaneCollapsed}
			>
				{statusLabel}
			</Tag>
			{#if !detailedPr?.closedAt && checksStatus !== null}
				<Tag
					icon={checksIcon}
					style={checksColor}
					kind={checksIcon == 'success-small' ? 'solid' : 'soft'}
					clickable
					verticalOrientation={isLaneCollapsed}
					on:mousedown={fetchChecks}
					help={`Updated ${$lastChecksFetch}`}
				>
					{checksText}
				</Tag>
			{/if}
			<Tag
				icon="open-link"
				style="ghost"
				kind="solid"
				clickable
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
					wide
					projectId={project.id}
					disabled={isFetchingChecks || pr?.draft || mergeableState != 'clean'}
					loading={isMerging}
					help="Merge pull request and refresh"
					on:click={async (e) => {
						if (!pr) return;
						isMerging = true;
						const method = e.detail.method;
						try {
							await githubService.merge(pr.number, method);
						} catch (err) {
							console.error(err);
							toasts.error('Failed to merge pull request');
						} finally {
							isMerging = false;
							baseBranchService.fetchFromTarget();
							branchService.reloadVirtualBranches();
							updateDetailsAndChecks();
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
		padding: var(--size-14);
	}

	.pr-title {
		color: var(--clr-scale-ntrl-0);
		margin-bottom: var(--size-12);
		margin-right: var(--size-28);
		user-select: text;
		cursor: text;
	}

	.pr-tags {
		display: flex;
		gap: var(--size-4);
	}

	.pr-actions {
		margin-top: var(--size-14);
		display: flex;
	}

	.floating-button {
		position: absolute;
		right: var(--size-6);
		top: var(--size-6);
	}
</style>
