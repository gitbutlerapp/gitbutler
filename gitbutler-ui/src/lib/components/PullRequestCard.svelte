<script lang="ts">
	import MergeButton from './MergeButton.svelte';
	import Tag, { type TagColor } from './Tag.svelte';
	import ViewPrContextMenu from '$lib/components/ViewPrContextMenu.svelte';
	import * as toasts from '$lib/utils/toasts';
	import { openExternalUrl } from '$lib/utils/url';
	import { onDestroy } from 'svelte';
	import type { BranchService } from '$lib/branches/service';
	import type { GitHubService } from '$lib/github/service';
	import type { ChecksStatus } from '$lib/github/types';
	import type { Branch } from '$lib/vbranches/types';
	import type iconsJson from '../icons/icons.json';

	export let isLaneCollapsed: boolean;
	export let branchService: BranchService;
	export let branch: Branch;
	export let githubService: GitHubService;
	export let projectId: string;
	export let isUnapplied = false;

	let isMerging = false;
	let isFetching = false;
	// Null means we successfully checked for
	let checksStatus: ChecksStatus | null | undefined;

	$: pr$ = githubService.get(branch.upstreamName);

	function updateContextMenu(copyablePrUrl: string) {
		if (popupMenu) popupMenu.$destroy();
		return new ViewPrContextMenu({
			target: document.body,
			props: { prUrl: copyablePrUrl }
		});
	}

	async function fetchPrStatus() {
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
		setTimeout(() => fetchPrStatus(), timeUntilUdate * 1000);
	}

	$: checksIcon = getChecksIcon(checksStatus);
	$: checksColor = getChecksColor(checksStatus);
	$: statusIcon = getStatusIcon();
	$: statusColor = getStatusColor();
	$: statusLabel = getPrStatusLabel();

	$: if ($pr$) fetchPrStatus();

	function getChecksColor(status: ChecksStatus): TagColor {
		if (status?.error) return 'error';
		if (!status) return 'light';
		if (status && !status.hasChecks) return 'ghost';
		if (status.completed) {
			return status.success ? 'success' : 'error';
		}

		return 'warning';
	}

	function getChecksIcon(status: ChecksStatus): keyof typeof iconsJson | undefined {
		if (isFetching) return 'spinner';

		if (status?.error) return 'error-small';
		if (!status) return;
		if (status && !status.hasChecks) return;
		if (status.completed) {
			return status.success ? 'success-small' : 'error-small';
		}

		return 'spinner';
	}

	function statusToTagText(status: ChecksStatus | undefined): string | undefined {
		if (status?.error) return 'error';
		if (!status) return;
		if (status && !status.hasChecks) return 'No checks';
		if (status.completed) {
			return status.success ? 'Checks passed' : 'Checks failed';
		}
		return 'Checks are running';
	}

	function getPrStatusLabel(): string | undefined {
		if ($pr$?.mergedAt) return 'merged';
		if ($pr$?.closedAt) return 'closed';
		return 'open';
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
	<div class="card pr-card">
		<div class="pr-title text-base-13 font-semibold">
			<span style="color: var(--clr-theme-scale-ntrl-50)">PR #{$pr$.number}:</span>
			{$pr$.title}
		</div>
		<div class="pr-tags">
			<Tag
				icon={statusIcon}
				color={statusColor}
				filled={statusLabel !== 'open'}
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
					on:click={fetchPrStatus}
					help="Refresh checks status"
				>
					{statusToTagText(checksStatus)}
				</Tag>
			{:else}
				<Tag color="light" verticalOrientation={isLaneCollapsed} help="Fetching checks status">
					No checks
				</Tag>
			{/if}
			<Tag
				icon="open-link"
				color="ghost"
				clickable
				border
				verticalOrientation={isLaneCollapsed}
				on:click={(e) => {
					const url = $pr$?.htmlUrl;
					if (url) openExternalUrl(url);
					e.preventDefault();
					e.stopPropagation();
				}}
				on:contextmenu={(e) => {
					e.preventDefault();
					popupMenu.openByMouse(e, undefined);
				}}
			>
				View PR
			</Tag>
		</div>

		<!--
        We can't show the merge button until we've waited for checks

        We use a octokit.checks.listForRef to find checks running for a PR, but right after
        creation this request succeeds but returns an empty array. So we need a better way
        determining "no checks will run for this PR" such that we can show the merge button
        immediately.
        -->
		{#if $pr$}
			<div class="pr-actions">
				<MergeButton
					{projectId}
					wide
					disabled={isFetching ||
						isUnapplied ||
						!$pr$ ||
						checksStatus == null ||
						!checksStatus?.success}
					loading={isMerging}
					help="Merge pull request and refresh"
					on:click={async (e) => {
						isMerging = true;
						const method = e.detail.method;
						try {
							if ($pr$) {
								await githubService.merge($pr$.number, method);
							}
						} catch {
							// TODO: Should we show the error from GitHub?
							toasts.error('Failed to merge pull request');
						} finally {
							isMerging = false;
							await fetchPrStatus();
							await branchService.reloadVirtualBranches();
						}
					}}
				/>
			</div>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.pr-card {
		padding: var(--space-14);
	}

	.pr-title {
		color: var(--clr-theme-scale-ntrl-0);
		margin-bottom: var(--space-12);
	}

	.pr-tags {
		display: flex;
		gap: var(--space-4);
	}

	.pr-actions {
		margin-top: var(--space-14);
		display: flex;
	}
</style>
