<script lang="ts">
	import MergeButton from './MergeButton.svelte';
	import InfoMessage from '../shared/InfoMessage.svelte';
	import { Project } from '$lib/backend/projects';
	import { BranchService } from '$lib/branches/service';
	import ViewPrContextMenu from '$lib/components/ViewPrContextMenu.svelte';
	import { GitHubService } from '$lib/github/service';
	import Button from '$lib/shared/Button.svelte';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { createTimeAgoStore } from '$lib/utils/timeAgo';
	import * as toasts from '$lib/utils/toasts';
	import { openExternalUrl } from '$lib/utils/url';
	import { BaseBranchService } from '$lib/vbranches/baseBranch';
	import { Branch } from '$lib/vbranches/types';
	import { distinctUntilChanged } from 'rxjs';
	import { mount, onDestroy, unmount } from 'svelte';
	import { derived, type Readable } from 'svelte/store';
	import type { ChecksStatus, DetailedPullRequest } from '$lib/github/types';
	import type iconsJson from '$lib/icons/icons.json';
	import type { MessageStyle } from '$lib/shared/InfoMessage.svelte';
	import type { ComponentColor } from '$lib/vbranches/types';

	type StatusInfo = {
		text: string;
		icon: keyof typeof iconsJson | undefined;
		style?: ComponentColor;
		messageStyle?: MessageStyle;
	};

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

	const distinctId = derived([branch], ([branch]) => {
		return branch.upstream?.sha || branch.head;
	});

	$: pr$ = githubService.getPr$($distinctId).pipe(
		// Only emit a new objcect if the modified timestamp has changed.
		distinctUntilChanged((prev, curr) => {
			return prev?.modifiedAt.getTime() === curr?.modifiedAt.getTime();
		})
	);
	$: if ($pr$) updateDetailsAndChecks();

	$: checksTagInfo = getChecksTagInfo(checksStatus, isFetchingChecks);
	$: infoProps = getInfoMessageInfo(detailedPr, mergeableState, checksStatus, isFetchingChecks);
	$: prStatusInfo = getPrStatusInfo(detailedPr);

	async function updateDetailsAndChecks() {
		if (!$pr$) return;
		if (!isFetchingDetails) await updateDetailedPullRequest($pr$.sha, true);
		if (!isFetchingChecks) await fetchChecks();
	}

	async function updateDetailedPullRequest(targetBranchSha: string, skipCache: boolean) {
		detailsError = undefined;
		isFetchingDetails = true;
		try {
			detailedPr = await githubService.getDetailedPr(targetBranchSha, skipCache);
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
		setTimeout(async () => await updateDetailsAndChecks(), timeUntilUdate * 1000);
	}

	function getChecksCount(status: ChecksStatus): string {
		if (!status) return 'Running checks';

		const finished = status.finished || 0;
		const skipped = status.skipped || 0;
		const total = (status.totalCount || 0) - skipped;

		return `Checks completed ${finished}/${total}`;
	}

	function getChecksTagInfo(
		status: ChecksStatus | null | undefined,
		fetching: boolean
	): StatusInfo {
		if (checksError || detailsError) {
			return { style: 'error', icon: 'warning-small', text: 'Failed to load' };
		}

		if (fetching || !status) {
			return { style: 'neutral', icon: 'spinner', text: 'Checks' };
		}

		if (status.completed) {
			const style = status.success ? 'success' : 'error';
			const icon = status.success ? 'success-small' : 'error-small';
			const text = status.success ? 'Checks passed' : 'Checks failed';
			return { style, icon, text };
		}

		return {
			style: status.failed > 0 ? 'error' : 'warning',
			icon: 'spinner',
			text: getChecksCount(status)
		};
	}

	function getPrStatusInfo(pr: DetailedPullRequest | undefined): StatusInfo {
		if (!pr) {
			return { text: 'Status', icon: 'spinner', style: 'neutral' };
		}

		if (pr?.mergedAt) {
			return { text: 'Merged', icon: 'merged-pr-small', style: 'purple' };
		}

		if (pr?.closedAt) {
			return { text: 'Closed', icon: 'closed-pr-small', style: 'error' };
		}

		if (pr?.draft) {
			return { text: 'Draft', icon: 'draft-pr-small', style: 'neutral' };
		}

		return { text: 'Open', icon: 'pr-small', style: 'success' };
	}

	function getInfoMessageInfo(
		pr: DetailedPullRequest | undefined,
		mergeableState: string | undefined,
		checksStatus: ChecksStatus | null | undefined,
		isFetchingChecks: boolean
	): StatusInfo | undefined {
		if (mergeableState === 'blocked' && !checksStatus && !isFetchingChecks) {
			return {
				icon: 'error',
				messageStyle: 'error',
				text: 'Merge is blocked due to pending reviews or missing dependencies. Resolve the issues before merging.'
			};
		}

		if (checksStatus?.completed) {
			if (pr?.draft) {
				return {
					icon: 'warning',
					messageStyle: 'neutral',
					text: 'This pull request is still a work in progress. Draft pull requests cannot be merged.'
				};
			}

			if (mergeableState === 'unstable') {
				return {
					icon: 'warning',
					messageStyle: 'warning',
					text: 'Your PR is causing instability or errors in the build or tests. Review the checks and fix the issues before merging.'
				};
			}

			if (mergeableState === 'dirty') {
				return {
					icon: 'warning',
					messageStyle: 'warning',
					text: 'Your PR has conflicts that must be resolved before merging.'
				};
			}

			if (mergeableState === 'blocked' && !isFetchingChecks) {
				return {
					icon: 'error',
					messageStyle: 'error',
					text: 'Merge is blocked due to failing checks. Resolve the issues before merging.'
				};
			}
		}
	}

	function updateContextMenu(copyablePrUrl: string) {
		if (popupMenu) unmount(popupMenu);
		return mount(ViewPrContextMenu, {
			target: document.body,
			props: { prUrl: copyablePrUrl }
		});
	}

	$: popupMenu = updateContextMenu($pr$?.htmlUrl || '');

	onDestroy(() => {
		if (popupMenu) {
			unmount(popupMenu);
		}
	});
</script>

{#if $pr$}
	{@const pr = $pr$}
	<div class="card pr-card">
		<div class="floating-button">
			<Button
				icon="update-small"
				size="tag"
				style="ghost"
				outline
				loading={isFetchingDetails || isFetchingChecks}
				help={$lastDetailsFetch ? 'Updated ' + $lastDetailsFetch : ''}
				on:click={async () => {
					await updateDetailsAndChecks();
				}}
			/>
		</div>
		<div class="pr-title text-base-13 text-semibold">
			<span style="color: var(--clr-scale-ntrl-50)">PR #{pr.number}:</span>
			{pr.title}
		</div>
		<div class="pr-tags">
			<Button
				size="tag"
				clickable={false}
				icon={prStatusInfo.icon}
				style={prStatusInfo.style}
				kind={prStatusInfo.text !== 'Open' && prStatusInfo.text !== 'Status' ? 'solid' : 'soft'}
			>
				{prStatusInfo.text}
			</Button>
			{#if !detailedPr?.closedAt && checksStatus !== null}
				<Button
					size="tag"
					clickable={false}
					icon={checksTagInfo.icon}
					style={checksTagInfo.style}
					kind={checksTagInfo.icon === 'success-small' ? 'solid' : 'soft'}
				>
					{checksTagInfo.text}
				</Button>
			{/if}
			<Button
				size="tag"
				icon="open-link"
				style="ghost"
				outline
				shrinkable
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
			</Button>
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
				{#if infoProps}
					<InfoMessage icon={infoProps.icon} filled outlined={false} style={infoProps.messageStyle}>
						<svelte:fragment slot="content">
							{infoProps.text}
						</svelte:fragment>
					</InfoMessage>
				{/if}

				<MergeButton
					wide
					projectId={project.id}
					disabled={isFetchingChecks ||
						isFetchingDetails ||
						pr?.draft ||
						(mergeableState !== 'clean' && mergeableState !== 'unstable')}
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
							baseBranchService.fetchFromRemotes();
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
		padding: 14px;
		margin-bottom: 8px;
	}

	.pr-title {
		color: var(--clr-scale-ntrl-0);
		margin-bottom: 12px;
		margin-right: 28px;
		user-select: text;
		cursor: text;
	}

	.pr-tags {
		display: flex;
		gap: 4px;
	}

	.pr-actions {
		margin-top: 14px;
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.floating-button {
		position: absolute;
		right: 6px;
		top: 6px;
	}
</style>
