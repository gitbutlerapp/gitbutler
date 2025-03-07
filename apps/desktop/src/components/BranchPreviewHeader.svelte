<script lang="ts">
	import { BranchController } from '$lib/branches/branchController';
	import { BranchListingService } from '$lib/branches/branchListing';
	import { getForge } from '$lib/forge/interface/forge';
	import { ModeService } from '$lib/mode/modeService';
	import { Project } from '$lib/project/project';
	import { openExternalUrl } from '$lib/utils/url';
	import { getContext } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import SeriesLabelsRow from '@gitbutler/ui/SeriesLabelsRow.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import { error } from '@gitbutler/ui/toasts';
	import type { BranchData } from '$lib/branches/branch';
	import type { PullRequest } from '$lib/forge/interface/types';
	import { goto } from '$app/navigation';

	interface Props {
		localBranch: BranchData | undefined;
		remoteBranch: BranchData | undefined;
		pr: PullRequest | undefined;
	}

	const { localBranch, remoteBranch, pr }: Props = $props();

	const branch = $derived(remoteBranch || localBranch!);
	const upstream = $derived(remoteBranch?.givenName);

	const branchController = getContext(BranchController);
	const branchListingService = getContext(BranchListingService);
	const project = getContext(Project);
	const forge = getForge();
	const modeSerivce = getContext(ModeService);
	const mode = modeSerivce.mode;
	const forgeBranch = $derived(upstream ? $forge?.branch(upstream) : undefined);

	const listingDetails = $derived(branchListingService.getBranchListingDetails(branch.givenName));
	const stackBranchNames = $derived.by(() => {
		if ($listingDetails?.virtualBranch) return $listingDetails.virtualBranch.stackBranches;
		if (pr) return [pr.title];
		if (branch) return [branch.givenName];
		return [];
	});

	let isApplying = $state(false);
	let isDeleting = $state(false);
	let deleteBranchModal = $state<Modal>();
</script>

<div class="header__wrapper">
	<div class="header card">
		<div class="header__info">
			<SeriesLabelsRow series={stackBranchNames} showRestAmount />
			<div class="header__remote-branch">
				{#if remoteBranch}
					<Tooltip text="At least some of your changes have been pushed">
						<Badge size="tag" icon="branch-small" style="neutral">
							{localBranch ? 'local and remote' : 'remote'}
						</Badge>
					</Tooltip>

					{#if forgeBranch}
						<Button
							size="tag"
							icon="open-link"
							kind="outline"
							shrinkable
							onclick={(e: MouseEvent) => {
								const url = forgeBranch.url;
								if (url) openExternalUrl(url);
								e.preventDefault();
								e.stopPropagation();
							}}
						>
							{branch.displayName}
						</Button>
					{/if}
				{:else}
					<Badge size="tag" icon="branch-small" style="neutral">local</Badge>
				{/if}
				{#if pr?.htmlUrl}
					<Button
						size="tag"
						icon="pr-small"
						kind="outline"
						onclick={(e: MouseEvent) => {
							const url = pr?.htmlUrl;
							if (url) openExternalUrl(url);
							e.preventDefault();
							e.stopPropagation();
						}}
					>
						View PR
					</Button>
				{/if}
			</div>
		</div>
		<div class="header__actions">
			<div class="header__buttons">
				<Button
					kind="outline"
					tooltip="Restores these changes into your working directory"
					icon="plus-small"
					loading={isApplying}
					disabled={$mode?.type !== 'OpenWorkspace'}
					onclick={async () => {
						isApplying = true;
						try {
							if (localBranch) {
								await branchController.createvBranchFromBranch(
									localBranch.name,
									remoteBranch?.name
								);
							} else {
								await branchController.createvBranchFromBranch(
									remoteBranch!.name,
									undefined,
									pr?.number
								);
							}
							goto(`/${project.id}/board`);
						} catch (e) {
							const err = 'Failed to apply branch';
							error(err);
							console.error(err, e);
						} finally {
							isApplying = false;
						}
					}}
				>
					Apply
				</Button>
				<Button
					kind="outline"
					tooltip="Deletes the local branch. If this branch is also present on a remote, it will not be deleted there."
					icon="bin-small"
					loading={isDeleting}
					disabled={!localBranch}
					onclick={async () => {
						if (localBranch) {
							deleteBranchModal?.show(branch);
						}
					}}
				>
					Delete local
				</Button>
			</div>
		</div>
	</div>
	<div class="header__top-overlay"></div>
</div>

<Modal
	width="small"
	bind:this={deleteBranchModal}
	onSubmit={async (close) => {
		try {
			isDeleting = true;
			await branchController.deleteLocalBranch(branch.name, branch.givenName);
		} catch (e) {
			const err = 'Failed to delete local branch';
			error(err);
			console.error(err, e);
		} finally {
			isDeleting = false;
			close();
		}
		goto(`/${project.id}/board`);
	}}
>
	{#snippet children(branch)}
		Are you sure you want to delete <code class="code-string">{branch.name}</code>?
	{/snippet}
	{#snippet controls(close)}
		<Button kind="outline" onclick={close}>Cancel</Button>
		<Button style="error" type="submit" loading={isDeleting}>Delete</Button>
	{/snippet}
</Modal>

<style lang="postcss">
	.header__wrapper {
		z-index: var(--z-lifted);
		position: sticky;
		top: 12px;
	}
	.header {
		z-index: var(--z-lifted);
		position: relative;
		flex-direction: column;
		gap: 2px;
	}
	.header__top-overlay {
		z-index: var(--z-ground);
		position: absolute;
		top: -16px;
		left: 0;
		width: 100%;
		height: 20px;
		background: var(--clr-bg-2);
	}
	.header__info {
		display: flex;
		flex-direction: column;
		transition: margin var(--transition-slow);
		padding: 10px;
		gap: 10px;
		overflow: hidden;
	}
	.header__actions {
		display: flex;
		gap: 4px;
		background: var(--clr-bg-2);
		padding: 14px;
		justify-content: flex-end;
		border-radius: 0 0 var(--radius-m) var(--radius-m);
		user-select: none;
	}
	.header__buttons {
		display: flex;
		position: relative;
		gap: 4px;
	}

	.header__remote-branch {
		color: var(--clr-scale-ntrl-50);
		padding-left: 2px;
		padding-right: 2px;
		display: flex;
		gap: 4px;
		text-overflow: ellipsis;
		overflow-x: hidden;
		white-space: nowrap;
		align-items: center;
	}
</style>
