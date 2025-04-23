<script lang="ts">
	import PullRequestPolling from '$components/PullRequestPolling.svelte';
	import { writeClipboard } from '$lib/backend/clipboard';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { getContext } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';
	import type { MessageStyle } from '$components/InfoMessage.svelte';
	import type { DetailedPullRequest } from '$lib/forge/interface/types';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import type { ComponentColorType } from '@gitbutler/ui/utils/colorTypes';
	import type { Snippet } from 'svelte';

	type StatusInfo = {
		text: string;
		icon: keyof typeof iconsJson | undefined;
		style?: ComponentColorType;
		messageStyle?: MessageStyle;
		tooltip?: string;
	};

	type ButtonStatus = {
		disabled: boolean;
		tooltip?: string;
	};

	interface Props {
		branchName: string;
		poll?: boolean;
		prNumber: number;
		isPushed?: boolean;
		hasParent?: boolean;
		baseIsTargetBranch?: boolean;
		parentIsPushed?: boolean;
		hasChecks?: boolean;
		checks?: Snippet<[DetailedPullRequest]>;
		button?: Snippet<
			[{ pr: DetailedPullRequest; mergeStatus: ButtonStatus; reopenStatus: ButtonStatus }]
		>;
	}

	const {
		poll,
		prNumber,
		isPushed,
		hasParent,
		baseIsTargetBranch,
		parentIsPushed,
		hasChecks,
		checks,
		button
	}: Props = $props();

	let contextMenuEl = $state<ReturnType<typeof ContextMenu>>();
	let container = $state<HTMLElement>();

	const forge = getContext(DefaultForgeFactory);
	const prService = $derived(forge.current.prService);

	const prResult = $derived(prService?.get(prNumber));
	const pr = $derived(prResult?.current.data);

	const parentResult = $derived(stackService.branchParentByName(projectId, stackId, branchName));
	const parent = $derived(parentResult.current.data);
	const hasParent = $derived(!!parent);
	const parentBranchDetailsResult = $derived(
		parent ? stackService.branchDetails(projectId, stackId, parent.name) : undefined
	);
	const parentBranchDetails = $derived(parentBranchDetailsResult?.current.data);
	const parentIsPushed = $derived(parentBranchDetails?.pushStatus !== 'completelyUnpushed');
	const childResult = $derived(stackService.branchChildByName(projectId, stackId, branchName));
	const child = $derived(childResult.current.data);

	const baseBranchResponse = $derived(baseBranchService.baseBranch(projectId));
	const baseBranch = $derived(baseBranchResponse.current.data);
	const baseBranchRepoResponse = $derived(baseBranchService.repo(projectId));
	const baseBranchRepo = $derived(baseBranchRepoResponse.current.data);
	const repoResult = $derived(repoService?.getInfo());
	const repoInfo = $derived(repoResult?.current.data);

	let shouldUpdateTargetBaseBranch = $state(false);
	$effect(() => {
		shouldUpdateTargetBaseBranch = repoInfo?.deleteBranchAfterMerge === false && !!child?.prNumber;
	});

	const baseIsTargetBranch = $derived.by(() => {
		if (forge.current.name === 'gitlab') return true;
		return pr
			? baseBranch?.shortName === pr.baseBranch && baseBranchRepo?.hash === pr.baseRepo?.hash
			: false;
	});

	const prLoading = $state(false);

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
		} else if (prLoading) {
			tooltip = 'Reloading pull request data';
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
	{#if poll}
		<PullRequestPolling number={pr.number} />
	{/if}

	<ContextMenu bind:this={contextMenuEl} rightClickTrigger={container}>
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
					writeClipboard(pr.htmlUrl);
					contextMenuEl?.close();
				}}
			/>
			<ContextMenuItem
				label="Refetch status"
				onclick={() => {
					prService?.fetch(pr.number, { forceRefetch: true });
					contextMenuEl?.close();
				}}
			/>
		</ContextMenuSection>
		{#if hasChecks}
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
						writeClipboard(`${pr.htmlUrl}/checks`);
						contextMenuEl?.close();
					}}
				/>
			</ContextMenuSection>
		{/if}
	</ContextMenu>

	<div
		bind:this={container}
		role="article"
		class="review-card pr-card"
		oncontextmenu={(e: MouseEvent) => {
			e.preventDefault();
			e.stopPropagation();
			contextMenuEl?.open(e);
		}}
	>
		<div class="pr-actions">
			<Button
				kind="outline"
				size="tag"
				icon="copy-small"
				tooltip="Copy PR link"
				onclick={() => {
					writeClipboard(pr.htmlUrl);
				}}
			/>
			<Button
				kind="outline"
				size="tag"
				icon="open-link"
				tooltip="Open PR in browser"
				onclick={() => {
					openExternalUrl(pr.htmlUrl);
				}}
			/>
		</div>

		<div class="text-13 text-semibold pr-row">
			<Icon name="github" />
			<h4 class="text-14 text-semibold">
				PR #{pr.number}
			</h4>
			<Badge
				reversedDirection
				size="icon"
				icon={prStatusInfo.icon}
				style={prStatusInfo.style}
				kind="soft"
				tooltip="PR status"
			>
				{prStatusInfo.text}
			</Badge>
		</div>
		<div class="text-12 pr-row">
			{#if !pr.closedAt && forge.current.checks}
				<div class="factoid">
					{@render checks?.(pr)}
				</div>
				<span class="seperator">•</span>
			{/if}
			<div class="factoid">
				{#if pr.reviewers.length > 0}
					<span class="label">Reviewers:</span>
					<div class="avatar-group-container">
						<AvatarGroup avatars={pr.reviewers} />
					</div>
				{:else}
					<span class="label italic">No reviewers</span>
				{/if}
			</div>
			<span class="seperator">•</span>
			<div class="factoid">
				<span class="label">
					<Icon name="chat-small" />
				</span>
				<span>{pr.commentsCount}</span>
			</div>
		</div>

		<div class="pr-row">
			{#if pr.state === 'open'}
				<MergeButton
					wide
					{projectId}
					disabled={mergeStatus.disabled}
					tooltip={mergeStatus.tooltip}
					loading={isMerging}
					onclick={async (method) => {
						if (!pr) return;
						isMerging = true;
						try {
							await prService?.merge(method, pr.number);

							// In a stack, after merging, update the new bottom PR target
							// base branch to master if necessary
							if (baseBranch && shouldUpdateTargetBaseBranch && prService && child?.prNumber) {
								const targetBase = baseBranch.branchName.replace(`${baseBranch.remoteName}/`, '');
								await prService.update(child.prNumber, { targetBase });
							}

							await Promise.all([
								baseBranchService.fetchFromRemotes(projectId),
								vbranchService.refresh(),
								baseBranchService.refreshBaseBranch(projectId)
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
				<AsyncButton
					kind="outline"
					disabled={reopenStatus.disabled}
					tooltip={reopenStatus.tooltip}
					action={handleReopenPr}
				>
					Reopen pull request
				</AsyncButton>
			{/if}
		</div>
	</div>
{/if}

<style lang="postcss">
	.pr-row {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 6px;
	}

	.factoid {
		display: flex;
		align-items: center;
		gap: 4px;

		> .label {
			color: var(--clr-text-2);

			&.italic {
				font-style: italic;
			}
		}
	}

	.seperator {
		transform: translateY(-1.5px);
		color: var(--clr-text-3);
	}

	.pr-actions {
		position: absolute;
		top: 8px;
		right: 8px;
		display: flex;
		gap: 4px;
	}
</style>
