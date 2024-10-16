<script lang="ts">
	import MergeButton from './MergeButton.svelte';
	import InfoMessage from '../shared/InfoMessage.svelte';
	import { Project } from '$lib/backend/projects';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import { getGitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
	import { getGitHostPrService } from '$lib/gitHost/interface/gitHostPrService';
	import * as toasts from '$lib/utils/toasts';
	import { openExternalUrl } from '$lib/utils/url';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import { type ComponentColor } from '@gitbutler/ui/utils/colorTypes';
	import type { GitHostPrMonitor } from '$lib/gitHost/interface/gitHostPrMonitor';
	import type { DetailedPullRequest } from '$lib/gitHost/interface/types';
	import type { MessageStyle } from '$lib/shared/InfoMessage.svelte';
	import type iconsJson from '@gitbutler/ui/data/icons.json';

	interface Props {
		pr: DetailedPullRequest;
		prMonitor?: GitHostPrMonitor;
		sourceBranch: string;
	}

	const { pr, prMonitor, sourceBranch }: Props = $props();

	type StatusInfo = {
		text: string;
		icon: keyof typeof iconsJson | undefined;
		style?: ComponentColor;
		messageStyle?: MessageStyle;
	};

	const vbranchService = getContext(VirtualBranchService);
	const baseBranchService = getContext(BaseBranchService);
	const project = getContext(Project);

	const gitHost = getGitHost();
	const gitHostListingService = getGitHostListingService();

	const prService = getGitHostPrService();

	const checksMonitor = $derived(sourceBranch ? $gitHost?.checksMonitor(sourceBranch) : undefined);
	// This PR has been loaded on demand, and contains more details than the version
	// obtained when listing them.
	const checks = $derived(checksMonitor?.status);

	// While the pr monitor is set to fetch updates by interval, we want
	// frequent updates while checks are running.
	// $effect(() => {
	// 	if ($checks) prMonitor?.refresh();
	// });

	let isMerging = $state(false);

	const mrLoading = $derived(prMonitor?.loading);
	const checksLoading = $derived(checksMonitor?.loading);

	const checksError = $derived(checksMonitor?.error);
	const detailsError = $derived(prMonitor?.error);

	const checksTagInfo: StatusInfo = $derived.by(() => {
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
			const text = $checks.completed
				? $checks.success
					? 'Checks passed'
					: 'Checks failed'
				: 'Checks running';
			return { style, icon, text };
		}
		if ($checksLoading) {
			return { style: 'neutral', icon: 'spinner', text: ' Checks' };
		}

		return { style: 'neutral', icon: undefined, text: 'No PR checks' };
	});

	const prStatusInfo: StatusInfo = $derived.by(() => {
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
	});

	const infoProps: StatusInfo | undefined = $derived.by(() => {
		const mergeableState = pr?.mergeableState;
		if (mergeableState === 'blocked' && !$checks && !$checksLoading) {
			return {
				icon: 'error',
				messageStyle: 'error',
				text: 'Merge is blocked due to pending reviews or missing dependencies. Resolve the issues before merging.'
			};
		}

		if ($checks?.completed) {
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

{#if pr}
	<div class="pr-header">
		<div class="text-13 text-semibold pr-header-title">
			<span style="color: var(--clr-scale-ntrl-50)">PR #{pr?.number}:</span>
			<span>{pr.title}</span>
		</div>
		<div class="pr-header-tags">
			<Button
				reversedDirection
				size="tag"
				clickable={false}
				icon={prStatusInfo.icon}
				style={prStatusInfo.style}
				kind={'soft'}
				tooltip="PR status"
			>
				{prStatusInfo.text}
			</Button>
			{#if !pr.closedAt && checksTagInfo}
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
				icon="open-link"
				size="tag"
				style="ghost"
				outline
				tooltip="Open in browser"
				onclick={() => {
					openExternalUrl(pr.htmlUrl);
				}}>View PR</Button
			>
		</div>

		<!--
        We can't show the merge button until we've waited for checks

        We use a octokit.checks.listForRef to find checks running for a PR, but right after
        creation this request succeeds but returns an empty array. So we need a better way
        determining "no checks will run for this PR" such that we can show the merge button
        immediately.
        -->
		{#if pr}
			<div class="pr-header-actions">
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
						pr.draft ||
						!pr.mergeable ||
						['dirty', 'unknown', 'blocked', 'behind'].includes(pr.mergeableState)}
					loading={isMerging}
					on:click={async (e) => {
						if (!pr) return;
						isMerging = true;
						const method = e.detail.method;
						try {
							await $prService?.merge(method, pr.number);
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

<style lang="postcss">
	.pr-header {
		position: relative;
		display: flex;
		flex-direction: column;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}

	.pr-header-title {
		color: var(--clr-scale-ntrl-0);
		padding: 14px 14px 12px 14px;
		user-select: text;
		cursor: text;
	}

	.pr-header-tags {
		display: flex;
		gap: 4px;
		padding: 0 14px 12px 14px;
	}

	.pr-header-actions {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 0 14px 12px 14px;
	}
</style>
