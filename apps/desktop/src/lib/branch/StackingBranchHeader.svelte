<script lang="ts">
	import BranchLabel from './BranchLabel.svelte';
	import StackingStatusIcon from './StackingStatusIcon.svelte';
	import { getColorFromBranchType } from './stackingUtils';
	import { Project } from '$lib/backend/projects';
	import { BaseBranch } from '$lib/baseBranch/baseBranch';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import StackingBranchHeaderContextMenu from '$lib/branch/StackingBranchHeaderContextMenu.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import { mapErrorToToast } from '$lib/gitHost/github/errorMap';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import { getGitHostListingService } from '$lib/gitHost/interface/gitHostListingService';
	import { getGitHostPrService } from '$lib/gitHost/interface/gitHostPrService';
	import { showError, showToast } from '$lib/notifications/toasts';
	import PullRequestButton from '$lib/pr/PullRequestButton.svelte';
	import StackingPullRequestCard from '$lib/pr/StackingPullRequestCard.svelte';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { sleep } from '$lib/utils/sleep';
	import { error } from '$lib/utils/toasts';
	import { openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { DetailedCommit, VirtualBranch, type CommitStatus } from '$lib/vbranches/types';
	import Button from '@gitbutler/ui/Button.svelte';
	import type { PullRequest } from '$lib/gitHost/interface/types';

	interface Props {
		name: string;
		upstreamName?: string;
		commits: DetailedCommit[];
	}

	const { name, upstreamName, commits }: Props = $props();

	let isLoading = $state(false);
	let descriptionVisible = $state(false);

	const branchStore = getContextStore(VirtualBranch);
	const branch = $derived($branchStore);

	const branchController = getContext(BranchController);
	const baseBranch = getContextStore(BaseBranch);
	const baseBranchService = getContext(BaseBranchService);
	const prService = getGitHostPrService();
	const gitListService = getGitHostListingService();
	const gitHost = getGitHost();
	const gitHostBranch = $derived(upstreamName ? $gitHost?.branch(upstreamName) : undefined);
	const project = getContext(Project);

	const baseBranchName = $derived($baseBranch.shortName);

	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let meatballButtonEl = $state<HTMLDivElement>();

	const branchColorType = $derived<CommitStatus>(branch.commits?.[0]?.status ?? 'local');
	const lineColor = $derived(getColorFromBranchType(branchColorType));

	// Pretty cumbersome way of getting the PR number, would be great if we can
	// make it more concise somehow.
	const hostedListingServiceStore = getGitHostListingService();
	const prStore = $derived($hostedListingServiceStore?.prs);
	const prs = $derived(prStore ? $prStore : undefined);

	const listedPr = $derived(prs?.find((pr) => pr.sourceBranch === upstreamName));
	const prNumber = $derived(listedPr?.number);

	const prMonitor = $derived(prNumber ? $prService?.prMonitor(prNumber) : undefined);
	const pr = $derived(prMonitor?.pr);

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
			title = name;
			body = pullRequestTemplateBody;
		} else {
			// In case of a single commit, use the commit summary and description for the title and
			// description of the PR.
			if (commits.length === 1) {
				const commit = commits[0];
				title = commit?.descriptionTitle ?? '';
				body = commit?.descriptionBody ?? '';
			} else {
				title = name;
				body = '';
			}
		}

		isLoading = true;
		try {
			let upstreamBranchName: string | undefined = upstreamName;

			if (commits.some((c) => !c.isRemote)) {
				const firstPush = !branch.upstream;
				await branchController.pushBranch(branch.id, branch.requiresForce, true);
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

	function editDescription(_description: string) {
		// branchController.updateBranchDescription(branch.id, description);
	}

	function addDescription() {
		descriptionVisible = true;
	}
</script>

<div class="branch-header">
	<div class="branch-info">
		<StackingStatusIcon icon="tick-small" iconColor="#fff" color={lineColor} gap={false} lineTop />
		<div class="text-14 text-bold branch-info__name">
			<span class="remote-name">{$baseBranch.remoteName ?? 'origin'}/</span>
			<BranchLabel {name} onChange={(name) => editTitle(name)} />
			<Button
				size="tag"
				icon="open-link"
				style="ghost"
				onclick={(e: MouseEvent) => {
					const url = gitHostBranch?.url;
					if (url) openExternalUrl(url);
					e.preventDefault();
					e.stopPropagation();
				}}
			></Button>
		</div>
		<div class="branch-info__btns">
			<Button
				size="tag"
				icon="kebab"
				style="ghost"
				bind:el={meatballButtonEl}
				onclick={() => {
					contextMenu?.toggle();
				}}
			></Button>
			<StackingBranchHeaderContextMenu
				bind:contextMenuEl={contextMenu}
				target={meatballButtonEl}
				headName={name}
				{addDescription}
			/>
		</div>
	</div>
	{#if descriptionVisible}
		<div class="branch-info__description">
			<div class="branch-action__line" style:--bg-color={lineColor}></div>
			<BranchLabel
				name={branch.description}
				onChange={(description) => editDescription(description)}
			/>
		</div>
	{/if}
	<div class="branch-action">
		<div class="branch-action__line" style:--bg-color={lineColor}></div>
		<div class="branch-action__body">
			{#if $pr}
				<StackingPullRequestCard pr={$pr} {prMonitor} sourceBranch={$pr.sourceBranch} />
			{:else}
				<PullRequestButton
					click={async ({ draft }) => await createPr({ draft })}
					disabled={commits.length === 0 || !$gitHost || !$prService}
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
			display: flex;
			align-items: stretch;
			justify-content: start;
			padding: 8px 16px;
			flex-grow: 1;
		}

		& .branch-info__btns {
			display: flex;
			gap: 0.25rem;
		}

		.remote-name {
			margin-top: 3px;
			color: var(--clr-scale-ntrl-60);
		}
	}

	.branch-info__description {
		width: 100%;
		display: flex;
		justify-content: flex-start;
		align-items: stretch;
	}

	.branch-action {
		width: 100%;
		display: flex;
		justify-content: flex-start;
		align-items: stretch;

		.branch-action__body {
			width: 100%;
			padding: 4px 12px 12px 0px;
		}
	}

	.branch-action__line {
		margin: 0 22px 0 22.5px;
		border-left: 2px solid var(--bg-color, var(--clr-border-3));
	}
</style>
