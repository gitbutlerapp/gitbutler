<script lang="ts">
	import MergeButton from './MergeButton.svelte';
	import PrDetailsModal from './PrDetailsModal.svelte';
	import InfoMessage from '../shared/InfoMessage.svelte';
	import { Project } from '$lib/backend/projects';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { getGitHostChecksMonitor } from '$lib/gitHost/interface/gitHostChecksMonitor';
	import { getGitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
	import { getGitHostPrService } from '$lib/gitHost/interface/gitHostPrService';
	import * as toasts from '$lib/utils/toasts';
	import { openExternalUrl } from '$lib/utils/url';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import { type ComponentColor } from '@gitbutler/ui/utils/colorTypes';
	import { createTimeAgoStore } from '@gitbutler/ui/utils/timeAgo';
	import type { ChecksStatus } from '$lib/gitHost/interface/types';
	import type { MessageStyle } from '$lib/shared/InfoMessage.svelte';
	import type iconsJson from '@gitbutler/ui/data/icons.json';

	interface Props {
		upstreamName: string;
	}

	const { upstreamName }: Props = $props();

	type StatusInfo = {
		text: string;
		icon?: keyof typeof iconsJson | undefined;
		style?: ComponentColor;
		messageStyle?: MessageStyle;
	};

	const vbranchService = getContext(VirtualBranchService);
	const baseBranchService = getContext(BaseBranchService);
	const project = getContext(Project);

	let prDetailsModal = $state<ReturnType<typeof PrDetailsModal>>();

	const gitHostListingService = getGitHostListingService();
	const prStore = $derived($gitHostListingService?.prs);
	const prs = $derived(prStore ? $prStore : undefined);

	const listedPr = $derived(prs?.find((pr) => pr.sourceBranch === upstreamName));
	const prNumber = $derived(listedPr?.number);

	const prService = getGitHostPrService();
	const prMonitor = $derived(prNumber ? $prService?.prMonitor(prNumber) : undefined);

	const checksMonitor = getGitHostChecksMonitor();
	// This PR has been loaded on demand, and contains more details than the version
	// obtained when listing them.
	const pr = $derived(prMonitor?.pr);
	const checks = $derived($checksMonitor?.status);

	// While the pr monitor is set to fetch updates by interval, we want
	// frequent updates while checks are running.
	$effect(() => {
		if ($checks) prMonitor?.refresh();
	});

	let isMerging = $state(false);

	const lastFetch = $derived(prMonitor?.lastFetch);
	const timeAgo = $derived($lastFetch ? createTimeAgoStore($lastFetch) : undefined);

	const mrLoading = $derived(prMonitor?.loading);
	const checksLoading = $derived($checksMonitor?.loading);

	const checksError = $derived($checksMonitor?.error);
	const detailsError = $derived(prMonitor?.error);

	function getChecksCount(status: ChecksStatus): string {
		if (!status) return 'Running checks';

		const finished = status.finished || 0;
		const skipped = status.skipped || 0;
		const total = (status.totalCount || 0) - skipped;

		return `Checks completed ${finished}/${total}`;
	}

	const checksTagInfo: StatusInfo | undefined = $derived.by(() => {
		if ($checksError || $detailsError) {
			return { style: 'error', icon: 'warning-small', text: 'Failed to load' };
		}

		if ($checks) {
			const style = $checks.completed ? ($checks.success ? 'success' : 'error') : 'warning';
			const icon =
				$checks.completed && !$checksLoading
					? $checks.success
						? 'success-small'
						: 'error-small'
					: 'spinner';
			const text = $checks.completed ? 'Checks' : getChecksCount($checks);
			return { style, icon, text };
		}
		if ($checksLoading) {
			return { style: 'neutral', icon: 'spinner', text: 'Checks' };
		}
	});

	const prStatusInfo: StatusInfo = $derived.by(() => {
		if (!$pr) {
			return { text: 'Status', style: 'neutral' };
		}

		if ($pr?.mergedAt) {
			return { text: 'Merged', style: 'purple' };
		}

		if ($pr?.closedAt) {
			return { text: 'Closed', style: 'error' };
		}

		if ($pr?.draft) {
			return { text: 'Draft', style: 'neutral' };
		}

		return { text: 'Open', style: 'success' };
	});

	const infoProps: StatusInfo | undefined = $derived.by(() => {
		const mergeableState = $pr?.mergeableState;
		if (mergeableState === 'blocked' && !$checks && !$checksLoading) {
			return {
				icon: 'error',
				messageStyle: 'error',
				text: 'Merge is blocked due to pending reviews or missing dependencies. Resolve the issues before merging.'
			};
		}

		if ($checks?.completed) {
			if ($pr?.draft) {
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
			if (
				mergeableState === 'blocked' &&
				!$checksLoading &&
				$checks?.failed &&
				$checks.failed > 0
			) {
				return {
					icon: 'error',
					messageStyle: 'error',
					text: 'Merge is blocked due to failing checks. Resolve the issues before merging.'
				};
			}
		}
	});
</script>

{#if $pr}
	<div class="card pr-card">
		<div class="pr-title text-13 text-semibold">
			<span style="color: var(--clr-scale-ntrl-50)">PR #{$pr?.number}:</span>
			<span>{$pr.title}</span>
		</div>
		<div class="pr-tags">
			<Button
				size="tag"
				clickable={false}
				style={prStatusInfo.style}
				tooltip="PR status"
				kind={prStatusInfo.text !== 'Open' && prStatusInfo.text !== 'Status' ? 'solid' : 'soft'}
			>
				{prStatusInfo.text}
			</Button>
			{#if !$pr.closedAt && checksTagInfo}
				<Button
					size="tag"
					clickable={false}
					icon={checksTagInfo.icon}
					reversedDirection
					style={checksTagInfo.style}
					kind={checksTagInfo.icon === 'success-small' ? 'solid' : 'soft'}
				>
					{checksTagInfo.text}
				</Button>
			{/if}
			<Button
				size="tag"
				style="ghost"
				outline
				icon="description-small"
				onclick={() => {
					prDetailsModal?.show();
				}}
			>
				PR details
			</Button>
			<Button
				icon="open-link"
				size="tag"
				style="ghost"
				outline
				tooltip="Open in browser"
				onclick={() => {
					openExternalUrl($pr.htmlUrl);
				}}
			/>
			<Button
				icon="update-small"
				size="tag"
				style="ghost"
				outline
				loading={$mrLoading}
				tooltip={$timeAgo ? 'Updated ' + $timeAgo : ''}
				onclick={async () => {
					$checksMonitor?.update();
					prMonitor?.refresh();
				}}
			/>
		</div>

		<!--
        We can't show the merge button until we've waited for checks

        We use a octokit.checks.listForRef to find checks running for a PR, but right after
        creation this request succeeds but returns an empty array. So we need a better way
        determining "no checks will run for this PR" such that we can show the merge button
        immediately.
        -->
		{#if $pr}
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
					disabled={$mrLoading ||
						$checksLoading ||
						$pr.draft ||
						!$pr.mergeable ||
						['dirty', 'unknown', 'blocked', 'behind'].includes($pr.mergeableState)}
					loading={isMerging}
					on:click={async (e) => {
						if (!$pr) return;
						isMerging = true;
						const method = e.detail.method;
						try {
							await $prService?.merge(method, $pr.number);
							await baseBranchService.fetchFromRemotes();
							await Promise.all([
								prMonitor?.refresh(),
								$gitHostListingService?.refresh(),
								vbranchService.refresh(),
								baseBranchService.refresh()
							]);
						} catch (err) {
							console.error(err);
							toasts.error('Failed to merge pull request');
						} finally {
							isMerging = false;
						}
					}}
				/>
			</div>
		{/if}
	</div>
{/if}

{#if $pr}
	<PrDetailsModal bind:this={prDetailsModal} type="display" pr={$pr} />
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
</style>
