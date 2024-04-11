<script lang="ts">
	import IconButton from './IconButton.svelte';
	import InfoMessage, { type MessageStyle } from './InfoMessage.svelte';
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

	$: pr$ = githubService.getPr$($branch.upstreamName);
	$: if ($branch && $pr$) updateDetailsAndChecks();

	$: checksTagInfo = getChecksTagInfo(checksStatus, isFetchingChecks);
	$: infoMessageInfo = getInfoMessageInfo(
		detailedPr,
		mergeableState,
		checksStatus,
		isFetchingChecks
	);
	$: prStatusInfo = getPrStatusInfo(detailedPr);

	async function updateDetailsAndChecks() {
		if (!isFetchingDetails) {
			await updateDetailedPullRequest($pr$?.targetBranch, true);
		}

		if (!isFetchingChecks) {
			await fetchChecks();
		}
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
		} catch (e: any) {
			console.error(e);
			checksError = e.message;
			if (!e.message.includes('No commit found')) {
				toasts.error('Failed to fetch checks');
			}
		} finally {
			isFetchingChecks = false;
		}

		if (checksStatus) scheduleNextUpdate();
	}

	function scheduleNextUpdate() {
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

	function getChecksCount(status: ChecksStatus): string {
		if (!status) return 'Running checks';

		const skipped = status.skipped || 0;
		const total = (status.totalCount || 0) - skipped;
		const quieed = total - (status.queued || 0);

		return `Running checks ${quieed}/${total}`;
	}

	function getChecksTagInfo(
		status: ChecksStatus | null | undefined,
		fetching: boolean
	): {
		color: TagStyle;
		icon: keyof typeof iconsJson;
		text: string;
	} {
		if (checksError || detailsError) {
			return { color: 'error', icon: 'warning-small', text: 'Failed to load' };
		}

		if (fetching || !status) {
			return { color: 'neutral', icon: 'spinner', text: 'Checks' };
		}

		if (status.completed) {
			const color = status.success ? 'success' : 'error';
			const icon = status.success ? 'success-small' : 'error-small';
			const text = status.success ? 'Checks passed' : 'Checks failed';
			return { color, icon, text };
		}

		return {
			color: 'warning',
			icon: 'spinner',
			text: getChecksCount(status)
		};
	}

	function getPrStatusInfo(pr: DetailedPullRequest | undefined): {
		label: string;
		icon: keyof typeof iconsJson | undefined;
		color: TagStyle;
	} {
		if (!pr) {
			return { label: 'Status', icon: 'spinner', color: 'neutral' };
		}

		if (pr?.mergedAt) {
			return { label: 'Merged', icon: 'merged-pr-small', color: 'purple' };
		}

		if (pr?.closedAt) {
			return { label: 'Closed', icon: 'closed-pr-small', color: 'error' };
		}

		if (pr?.draft) {
			return { label: 'Draft', icon: 'draft-pr-small', color: 'neutral' };
		}

		return { label: 'Open', icon: 'pr-small', color: 'success' };
	}

	function getInfoMessageInfo(
		pr: DetailedPullRequest | undefined,
		mergeableState: string | undefined,
		checksStatus: ChecksStatus | null | undefined,
		isFetchingChecks: boolean
	):
		| {
				icon: keyof typeof iconsJson;
				style: MessageStyle;
				text: string;
		  }
		| undefined {
		if (mergeableState == 'blocked' && !checksStatus && !isFetchingChecks) {
			return {
				icon: 'error',
				style: 'error',
				text: 'Merge is blocked due to pending reviews or missing dependencies. Resolve the issues before merging.'
			};
		}

		if (checksStatus?.completed) {
			if (pr?.draft) {
				return {
					icon: 'warning',
					style: 'neutral',
					text: 'This pull request is still a work in progress. Draft pull requests cannot be merged.'
				};
			}

			if (mergeableState == 'unstable') {
				return {
					icon: 'warning',
					style: 'warn',
					text: 'Your PR is causing instability or errors in the build or tests. Review the checks and fix the issues before merging.'
				};
			}

			if (mergeableState == 'dirty') {
				return {
					icon: 'warning',
					style: 'warn',
					text: 'Your PR has conflicts that must be resolved before merging.'
				};
			}

			if (mergeableState == 'blocked' && !isFetchingChecks) {
				return {
					icon: 'error',
					style: 'error',
					text: 'Merge is blocked due to failing checks. Resolve the issues before merging.'
				};
			}
		}
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

{#if $pr$}
	{@const pr = $pr$}
	<div class="card pr-card">
		<div class="floating-button">
			<IconButton
				icon="update-small"
				size="m"
				loading={isFetchingDetails || isFetchingChecks}
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
				icon={prStatusInfo.icon}
				style={prStatusInfo.color}
				kind={prStatusInfo.label !== 'Open' && prStatusInfo.label !== 'Status' ? 'solid' : 'soft'}
				verticalOrientation={isLaneCollapsed}
			>
				{prStatusInfo.label}
			</Tag>
			{#if !detailedPr?.closedAt && checksStatus !== null}
				<Tag
					icon={checksTagInfo.icon}
					style={checksTagInfo.color}
					kind={checksTagInfo.icon == 'success-small' ? 'solid' : 'soft'}
					verticalOrientation={isLaneCollapsed}
				>
					{checksTagInfo.text}
				</Tag>
			{/if}
			<Tag
				icon="open-link"
				style="ghost"
				kind="solid"
				clickable
				shrinkable
				verticalOrientation={isLaneCollapsed}
				on:click={(e) => {
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
				{#if infoMessageInfo}
					<InfoMessage
						icon={infoMessageInfo.icon}
						filled
						outlined={false}
						style={infoMessageInfo.style}>{infoMessageInfo.text}</InfoMessage
					>
				{/if}

				<MergeButton
					wide
					projectId={project.id}
					disabled={isFetchingChecks ||
						isFetchingDetails ||
						pr?.draft ||
						(mergeableState != 'clean' && mergeableState != 'unstable')}
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
		flex-direction: column;
		gap: var(--size-8);
	}

	.floating-button {
		position: absolute;
		right: var(--size-6);
		top: var(--size-6);
	}
</style>
