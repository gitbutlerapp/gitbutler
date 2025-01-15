<script lang="ts">
	import ContextMenu from '$components/ContextMenu.svelte';
	import ContextMenuItem from '$components/ContextMenuItem.svelte';
	import ContextMenuSection from '$components/ContextMenuSection.svelte';
	import MergeButton from '$components/MergeButton.svelte';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { type ForgeChecksMonitor } from '$lib/forge/interface/forgeChecksMonitor';
	import { getForgeListingService } from '$lib/forge/interface/forgeListingService';
	import { getForgePrService } from '$lib/forge/interface/forgePrService';
	import { getForgeRepoService } from '$lib/forge/interface/forgeRepoService';
	import { showError } from '$lib/notifications/toasts';
	import { Project } from '$lib/project/projects';
	import { copyToClipboard } from '$lib/utils/clipboard';
	import { openExternalUrl } from '$lib/utils/url';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import type { MessageStyle } from '$components/InfoMessage.svelte';
	import type { ForgePrMonitor } from '$lib/forge/interface/forgePrMonitor';
	import type { DetailedPullRequest } from '$lib/forge/interface/types';
	import type { PatchSeries } from '$lib/vbranches/types';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import type { ComponentColorType } from '@gitbutler/ui/utils/colorTypes';

	interface Props {
		pr: DetailedPullRequest;
		isPushed: boolean;
		hasParent: boolean;
		parentIsPushed: boolean;
		child?: PatchSeries;
		checksMonitor?: ForgeChecksMonitor;
		prMonitor?: ForgePrMonitor;
		reloadPR: () => void;
		reopenPr: () => Promise<void>;
		openPrDetailsModal: () => void;
	}

	const {
		checksMonitor,
		child,
		hasParent,
		isPushed,
		openPrDetailsModal,
		parentIsPushed,
		pr,
		prMonitor,
		reloadPR,
		reopenPr
	}: Props = $props();

	type StatusInfo = {
		text: string;
		icon: keyof typeof iconsJson | undefined;
		style?: ComponentColorType;
		messageStyle?: MessageStyle;
		tooltip?: string;
	};

	let contextMenuEl = $state<ReturnType<typeof ContextMenu>>();
	let contextMenuTarget = $state<HTMLElement>();

	const vbranchService = getContext(VirtualBranchService);
	const baseBranchService = getContext(BaseBranchService);
	const baseBranch = getContextStore(BaseBranch);
	const repoService = getForgeRepoService();
	const project = getContext(Project);

	const forgeListingService = getForgeListingService();
	const prService = getForgePrService();

	const checks = $derived(checksMonitor?.status);
	const repoInfoStore = $derived($repoService?.info);
	const repoInfo = $derived(repoInfoStore && $repoInfoStore);
	let shouldUpdateTargetBaseBranch = $state(false);
	$effect(() => {
		shouldUpdateTargetBaseBranch = repoInfo?.deleteBranchAfterMerge === false && !!child?.prNumber;
	});

	const baseBranchRepo = $derived(baseBranchService.repo);
	const baseIsTargetBranch = $derived(
		pr
			? $baseBranch.shortName === pr.baseBranch && $baseBranchRepo?.hash === pr?.baseRepo?.hash
			: false
	);

	// While the pr monitor is set to fetch updates by interval, we want
	// frequent updates while checks are running.
	$effect(() => {
		if ($checks) {
			prMonitor?.refresh();
		}
	});

	let isMerging = $state(false);

	const mrLoading = $derived(prMonitor?.loading);
	const checksLoading = $derived(checksMonitor?.loading);
	let loading = $state(false);

	const checksError = $derived(checksMonitor?.error);
	const detailsError = $derived(prMonitor?.error);

	const checksTagInfo: StatusInfo = $derived.by(() => {
		if (!checksMonitor && pr.fork) {
			return {
				style: 'neutral',
				icon: 'info',
				text: 'No PR checks',
				tooltip: 'Checks for forked repos only available on the web.'
			};
		}

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
			return { style: 'neutral', icon: 'spinner', text: 'Checks' };
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

	const mergeStatus = $derived.by(() => {
		let disabled = true;
		let tooltip = undefined;
		if (isPushed && hasParent && !parentIsPushed) {
			tooltip = 'Remote parent branch seems to have been deleted';
		} else if (!baseIsTargetBranch) {
			tooltip = 'Pull request is not next in stack';
		} else if ($mrLoading) {
			tooltip = 'Reloading pull request data';
		} else if ($checksLoading) {
			tooltip = 'Reloading checks data';
		} else if (pr?.draft) {
			tooltip = 'Pull request is a draft';
		} else if (pr?.mergeableState === 'blocked') {
			tooltip = 'Pull request needs approval';
		} else if (pr?.mergeableState === 'unknown') {
			tooltip = 'Pull request mergeability is unknown';
		} else if (pr?.mergeableState === 'behind') {
			tooltip = 'Pull request base is too far behind';
		} else if (pr?.mergeableState === 'dirty') {
			tooltip = 'Pull request has conflicts';
		} else if (!pr?.mergeable) {
			tooltip = 'Pull request is not mergeable';
		} else {
			disabled = false;
		}
		return { disabled, tooltip };
	});

	const reopenStatus = $derived.by(() => {
		let disabled = true;
		let tooltip = undefined;
		if (isPushed && hasParent && !parentIsPushed) {
			tooltip = 'Remote parent branch seems to have been deleted';
		} else {
			disabled = false;
		}
		return { disabled, tooltip };
	});
</script>

{#if pr}
	<ContextMenu bind:this={contextMenuEl} rightClickTrigger={contextMenuTarget}>
		<ContextMenuSection>
			<ContextMenuItem
				label="Open in browser"
				onclick={() => {
					openExternalUrl(pr.htmlUrl);
					contextMenuEl?.close();
				}}
			/>
			<ContextMenuItem
				label="Copy link"
				onclick={() => {
					copyToClipboard(pr.htmlUrl);
					contextMenuEl?.close();
				}}
			/>
			<ContextMenuItem
				label="Show details"
				onclick={() => {
					openPrDetailsModal();
					contextMenuEl?.close();
				}}
			/>
			<ContextMenuItem
				label="Refetch status"
				onclick={() => {
					reloadPR();
					contextMenuEl?.close();
				}}
			/>
		</ContextMenuSection>
		{#if checksTagInfo}
			{#if checksTagInfo.text !== 'No PR checks' && checksTagInfo.text !== 'Checks'}
				<ContextMenuSection>
					<ContextMenuItem
						label="Open checks"
						onclick={() => {
							openExternalUrl(`${pr.htmlUrl}/checks`);
							contextMenuEl?.close();
						}}
					/>
					<ContextMenuItem
						label="Copy checks"
						onclick={() => {
							copyToClipboard(`${pr.htmlUrl}/checks`);
							contextMenuEl?.close();
						}}
					/>
				</ContextMenuSection>
			{/if}
		{/if}
	</ContextMenu>

	<div
		bind:this={contextMenuTarget}
		role="article"
		class="pr-header"
		oncontextmenu={(e: MouseEvent) => {
			e.preventDefault();
			e.stopPropagation();
			contextMenuEl?.open(e);
		}}
	>
		<div class="text-13 text-semibold pr-header-title">
			<span style="color: var(--clr-scale-ntrl-50)">PR #{pr?.number}:</span>
			<span>{pr?.title}</span>
		</div>
		<div class="pr-header-tags">
			<Badge
				reversedDirection
				size="tag"
				icon={prStatusInfo.icon}
				style={prStatusInfo.style}
				kind="soft"
				tooltip="PR status"
			>
				{prStatusInfo.text}
			</Badge>
			{#if !pr.closedAt && checksTagInfo}
				<Badge
					size="tag"
					icon={checksTagInfo.icon}
					style={checksTagInfo.style}
					kind={checksTagInfo.icon === 'success-small' ? 'solid' : 'soft'}
					tooltip={checksTagInfo.tooltip}
				>
					{checksTagInfo.text}
				</Badge>
			{/if}
			{#if pr.htmlUrl}
				<Button
					icon="open-link"
					size="tag"
					kind="outline"
					tooltip="Open in browser"
					onclick={() => {
						openExternalUrl(pr.htmlUrl);
					}}
				>
					View PR
				</Button>
			{/if}
		</div>

		<!--
        We can't show the merge button until we've waited for checks

        We use a octokit.checks.listForRef to find checks running for a PR, but right after
        creation this request succeeds but returns an empty array. So we need a better way
        determining "no checks will run for this PR" such that we can show the merge button
        immediately.
        -->
		<div class="pr-header-actions">
			{#if pr.state === 'open'}
				<MergeButton
					wide
					projectId={project.id}
					disabled={mergeStatus.disabled}
					tooltip={mergeStatus.tooltip}
					loading={isMerging}
					onclick={async (method) => {
						if (!pr) return;
						isMerging = true;
						try {
							await $prService?.merge(method, pr.number);

							// In a stack, after merging, update the new bottom PR target
							// base branch to master if necessary
							if (shouldUpdateTargetBaseBranch && $prService && child?.prNumber) {
								const targetBase = $baseBranch.branchName.replace(`${$baseBranch.remoteName}/`, '');
								await $prService.update(child.prNumber, { targetBase });
							}

							await Promise.all([
								baseBranchService.fetchFromRemotes(),
								prMonitor?.refresh(),
								$forgeListingService?.refresh(),
								vbranchService.refresh(),
								baseBranchService.refresh(),
								checksMonitor?.update()
							]);
						} catch (err) {
							console.error(err);
							showError('Failed to merge PR', err);
						} finally {
							isMerging = false;
						}
					}}
				/>
			{:else if !pr.merged}
				<Button
					kind="outline"
					disabled={reopenStatus.disabled}
					tooltip={reopenStatus.tooltip}
					{loading}
					onclick={async () => {
						loading = true;
						try {
							await reopenPr?.();
						} finally {
							loading = false;
						}
					}}
				>
					Reopen pull request
				</Button>
			{/if}
		</div>
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

		/* don't display if empty */
		&:empty {
			display: none;
		}
	}
</style>
