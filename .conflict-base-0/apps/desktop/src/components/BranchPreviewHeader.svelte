<script lang="ts">
	import { goto } from '$app/navigation';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { BranchService } from '$lib/branches/branchService.svelte';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { ModeService } from '$lib/mode/modeService';
	import { Project } from '$lib/project/project';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { getContext, inject } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import SeriesLabelsRow from '@gitbutler/ui/SeriesLabelsRow.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import { error } from '@gitbutler/ui/toasts';
	import type { BranchData } from '$lib/branches/branch';
	import type { PullRequest } from '$lib/forge/interface/types';

	interface Props {
		projectId: string;
		localBranch: BranchData | undefined;
		remoteBranch: BranchData | undefined;
		pr: PullRequest | undefined;
	}

	const { projectId, localBranch, remoteBranch, pr }: Props = $props();

	const branch = $derived(remoteBranch || localBranch!);
	const upstream = $derived(remoteBranch?.givenName);

	const [project, forge, modeSerivce, baseBranchService, branchService] = inject(
		Project,
		DefaultForgeFactory,
		ModeService,
		BaseBranchService,
		BranchService
	);

	const stackService = getContext(StackService);

	const mode = modeSerivce.mode;
	const forgeBranch = $derived(upstream ? forge.current.branch(upstream) : undefined);

	const detailsResult = $derived(branchService.get(projectId, branch.givenName));
	const details = $derived(detailsResult.current.data);
	const stackBranchNames = $derived.by(() => {
		if (details?.stack) return details.stack.branches;
		if (pr) return [pr.title];
		if (branch) return [branch.givenName];
		return [];
	});

	let isApplying = $state(false);
	let isDeleting = $state(false);
	let deleteBranchModal = $state<Modal>();

	async function createvBranchFromBranch(branch: string, remote?: string, prNumber?: number) {
		await stackService.createVirtualBranchFromBranch({
			projectId: project.id,
			branch,
			remote,
			prNumber
		});
		await baseBranchService.refreshBaseBranch(project.id);
	}

	async function deleteLocalBranch(refname: string, givenName: string) {
		await stackService.deleteLocalBranch({
			projectId: project.id,
			refname,
			givenName
		});
		await baseBranchService.refreshBaseBranch(project.id);
	}
</script>

<div class="header__wrapper">
	<div class="header card">
		<div class="header__info">
			<SeriesLabelsRow series={stackBranchNames} />
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
								await createvBranchFromBranch(localBranch.name, remoteBranch?.name);
							} else {
								await createvBranchFromBranch(remoteBranch!.name, undefined, pr?.number);
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
			await deleteLocalBranch(branch.name, branch.givenName);
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
		<p class="text-13 text-body">
			Are you sure you want to delete <code class="code-string">{branch.name}</code>?
		</p>
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
		padding: 10px;
		overflow: hidden;
		gap: 10px;
		transition: margin var(--transition-slow);
	}
	.header__actions {
		display: flex;
		justify-content: flex-end;
		padding: 14px;
		gap: 4px;
		border-radius: 0 0 var(--radius-m) var(--radius-m);
		background: var(--clr-bg-2);
	}
	.header__buttons {
		display: flex;
		position: relative;
		gap: 4px;
	}

	.header__remote-branch {
		display: flex;
		align-items: center;
		padding-right: 2px;
		padding-left: 2px;
		overflow-x: hidden;
		gap: 4px;
		color: var(--clr-scale-ntrl-50);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
