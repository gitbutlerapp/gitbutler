<script lang="ts">
	import BranchLabel from './BranchLabel.svelte';
	import StackingStatusIcon from './StackingStatusIcon.svelte';
	import { getColorFromBranchType, type BranchColor } from './stackingUtils';
	import { Project } from '$lib/backend/projects';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { mapErrorToToast } from '$lib/gitHost/github/errorMap';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import { getGitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
	import { getGitHostPrMonitor } from '$lib/gitHost/interface/gitHostPrMonitor';
	import { getGitHostPrService } from '$lib/gitHost/interface/gitHostPrService';
	import { showError, showToast } from '$lib/notifications/toasts';
	import PullRequestButton from '$lib/pr/PullRequestButton.svelte';
	import StackingPullRequestCard from '$lib/pr/StackingPullRequestCard.svelte';
	import { getBranchNameFromRef } from '$lib/utils/branch';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { sleep } from '$lib/utils/sleep';
	import { error } from '$lib/utils/toasts';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranch } from '$lib/vbranches/types';
	import Button from '@gitbutler/ui/Button.svelte';
	import type { PullRequest } from '$lib/gitHost/interface/types';

	let isLoading = $state(false);

	const branchStore = getContextStore(VirtualBranch);
	const branch = $derived($branchStore);

	const branchController = getContext(BranchController);
	const baseBranch = getContextStore(BaseBranch);
	const prMonitor = getGitHostPrMonitor();
	const baseBranchService = getContext(BaseBranchService);
	const prService = getGitHostPrService();
	const gitListService = getGitHostListingService();
	const gitHost = getGitHost();
	const project = getContext(Project);

	const baseBranchName = $derived($baseBranch.shortName);
	const pr = $derived($prMonitor?.pr);

	// TODO: Get Branch Status
	const branchType = $state<BranchColor>('integrated');
	const lineColor = $derived(getColorFromBranchType(branchType));

	interface CreatePrOpts {
		draft: boolean;
	}

	const defaultPrOpts: CreatePrOpts = {
		draft: true
	};

	async function createPr(createPrOpts: CreatePrOpts): Promise<PullRequest | undefined> {
		const opts = { ...defaultPrOpts, ...createPrOpts };
		if (!$gitHost) {
			error('Pull request service not available');
			return;
		}

		let title: string;
		let body: string;

		let pullRequestTemplateBody: string | undefined;
		const prTemplatePath = project.git_host.pullRequestTemplatePath;

		if (prTemplatePath) {
			pullRequestTemplateBody = await $prService?.pullRequestTemplateContent(
				prTemplatePath,
				project.id
			);
		}

		if (pullRequestTemplateBody) {
			title = branch.name;
			body = pullRequestTemplateBody;
		} else {
			// In case of a single commit, use the commit summary and description for the title and
			// description of the PR.
			if (branch.commits.length === 1) {
				const commit = branch.commits[0];
				title = commit?.descriptionTitle ?? '';
				body = commit?.descriptionBody ?? '';
			} else {
				title = branch.name;
				body = '';
			}
		}

		isLoading = true;
		try {
			let upstreamBranchName = branch.upstreamName;

			if (branch.commits.some((c) => !c.isRemote)) {
				const firstPush = !branch.upstream;
				const { refname, remote } = await branchController.pushBranch(
					branch.id,
					branch.requiresForce
				);
				upstreamBranchName = getBranchNameFromRef(refname, remote);

				if (firstPush) {
					// TODO: fix this hack for reactively available prService.
					await sleep(500);
				}
			}

			if (!baseBranchName) {
				error('No base branch name determined');
				return;
			}

			if (!upstreamBranchName) {
				error('No upstream branch name determined');
				return;
			}

			if (!$prService) {
				error('Pull request service not available');
				return;
			}

			await $prService.createPr({
				title,
				body,
				draft: opts.draft,
				baseBranchName,
				upstreamName: upstreamBranchName
			});
		} catch (err: any) {
			console.error(err);
			const toast = mapErrorToToast(err);
			if (toast) showToast(toast);
			else showError('Error while creating pull request', err);
		} finally {
			isLoading = false;
		}
		await $gitListService?.refresh();
		baseBranchService.fetchFromRemotes();
	}

	function editTitle(title: string) {
		branchController.updateBranchName(branch.id, title);
	}
</script>

<div class="branch-header">
	<div class="branch-info">
		<StackingStatusIcon icon="tick-small" color={branchType} gap={false} lineTop />
		<div class="text-14 text-bold branch-info__name">
			<span class="remote-name">{$baseBranch.remoteName ?? 'origin'}/</span>
			<BranchLabel name={branch.name} onChange={(name) => editTitle(name)} />
		</div>
		<div class="branch-info__btns">
			<Button icon="description" outline type="ghost" color="neutral" />
			<Button icon="edit-text" outline type="ghost" color="neutral" />
		</div>
	</div>
	<div class="branch-action">
		<div class="branch-action__line" style:--bg-color={lineColor}></div>
		<div class="branch-action__body">
			{#if $pr && branch.upstream?.givenName}
				<StackingPullRequestCard upstreamName={branch.upstream.givenName} />
			{:else}
				<PullRequestButton
					click={async ({ draft }) => await createPr({ draft })}
					disabled={branch.commits.length === 0 || !$gitHost || !$prService}
					tooltip={!$gitHost || !$prService
						? 'You can enable git host integration in the settings'
						: ''}
					loading={isLoading}
				/>
			{/if}
		</div>
	</div>
</div>

<style lang="postcss">
	.branch-header {
		display: flex;
		border-bottom: 1px solid var(--clr-border-2);
		display: flex;
		flex-direction: column;
	}

	.branch-info {
		padding: 0 13px;
		display: flex;
		justify-content: flex-start;
		align-items: center;

		& .branch-info__name {
			padding: 8px 16px;
			flex-grow: 1;
		}
		& .branch-info__btns {
			display: flex;
			gap: 0.25rem;
		}

		.remote-name {
			color: var(--clr-scale-ntrl-60);
			margin-right: -3px;
		}
	}

	.branch-action {
		width: 100%;
		display: flex;
		justify-content: flex-start;
		align-items: stretch;

		.branch-action__line {
			margin: 0 22px 0 22.5px;
			border-left: 2px solid var(--bg-color, var(--clr-border-3));
		}
		.branch-action__body {
			width: 100%;
			padding: 4px 12px 12px 0px;
		}
	}
</style>
