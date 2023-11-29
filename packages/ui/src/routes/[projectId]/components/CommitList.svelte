<script lang="ts" context="module">
	export type CommitType = 'local' | 'remote' | 'integrated';
</script>

<script lang="ts">
	import type { BaseBranch, Branch, Commit } from '$lib/vbranches/types';
	import PushButton from './PushButton.svelte';
	import type { GitHubIntegrationContext } from '$lib/github/types';
	import Button from '$lib/components/Button.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { DraggableCommit, DraggableFile, DraggableHunk } from '$lib/draggables';
	import Icon from '$lib/icons/Icon.svelte';
	import CommitListItem from './CommitListItem.svelte';
	import Link from '$lib/components/Link.svelte';
	import Tag from './Tag.svelte';
	import { open } from '@tauri-apps/api/shell';
	import toast from 'svelte-french-toast';
	import { sleep } from '$lib/utils/sleep';
	import type { PrService } from '$lib/github/pullrequest';

	export let branch: Branch;
	export let githubContext: GitHubIntegrationContext | undefined;
	export let base: BaseBranch | undefined | null;
	export let projectId: string;
	export let branchController: BranchController;
	export let type: CommitType;
	export let prService: PrService;
	export let readonly: boolean;

	export let acceptAmend: (commit: Commit) => (data: any) => boolean;
	export let acceptSquash: (commit: Commit) => (data: any) => boolean;
	export let onAmend: (data: DraggableFile | DraggableHunk) => void;
	export let onSquash: (commit: Commit) => (data: DraggableCommit) => void;
	export let resetHeadCommit: () => void;

	let isPushing: boolean;

	$: branchName = branch.upstream?.name.split('/').slice(-1)[0];
	$: headCommit = branch.commits[0];
	$: commits = branch.commits.filter((c) => {
		switch (type) {
			case 'local':
				return !c.isIntegrated && !c.isRemote;
			case 'remote':
				return !c.isIntegrated && c.isRemote;
			case 'integrated':
				return c.isIntegrated;
		}
	});
	$: pr$ = prService.get(branchName);
	$: prServiceState$ = prService.getState(branch.id);

	async function push(opts?: { createPr: boolean }) {
		isPushing = true;
		await branchController.pushBranch(branch.id, branch.requiresForce);
		if (opts?.createPr) {
			await sleep(500); // Needed by GitHub
			await createPr();
		}
		isPushing = false;
	}

	function branchUrl(target: BaseBranch | undefined | null, upstreamBranchName: string) {
		if (!target) return undefined;
		const baseBranchName = target.branchName.split('/')[1];
		const parts = upstreamBranchName.split('/');
		const branchName = parts[parts.length - 1];
		return `${target.repoBaseUrl}/compare/${baseBranchName}...${branchName}`;
	}

	async function createPr(): Promise<void> {
		if (githubContext && base?.branchName && branchName) {
			const pr = await prService.createPullRequest(
				githubContext,
				branchName,
				base.branchName.split('/').slice(-1)[0],
				branch.name,
				branch.notes
			);
			if (pr) {
				await prService.reload();
			}
			return;
		}
	}

	let expanded = true;
</script>

{#if commits.length > 0}
	<div class="wrapper">
		<button class="header" on:click={() => (expanded = !expanded)}>
			<div class="title text-base-12 text-bold">
				{#if type == 'local'}
					Local
				{:else if type == 'remote'}
					{#if branch.upstream}
						<Link
							target="_blank"
							rel="noreferrer"
							href={branchUrl(base, branch.upstream?.name)}
							class="inline-block max-w-full truncate"
						>
							{branch.upstream.name.split('refs/remotes/')[1]}
						</Link>
						{#if $pr$?.htmlUrl}
							<Tag
								icon="pr-small"
								color="neutral-light"
								clickable
								on:click={(e) => {
									const url = $pr$?.htmlUrl;
									if (url) open(url);
									e.preventDefault();
									e.stopPropagation();
								}}
							>
								PR
							</Tag>
						{/if}
					{/if}
				{:else if type == 'integrated'}
					Integrated
				{/if}
			</div>
			<div class="expander">
				<Icon name={expanded ? 'chevron-down' : 'chevron-top'} />
			</div>
		</button>
		{#if expanded}
			<div class="content-wrapper">
				<div class="commits">
					{#each commits as commit, idx (commit.id)}
						<div class="draggable-wrapper">
							<CommitListItem
								{commit}
								{base}
								{projectId}
								{readonly}
								{acceptAmend}
								{acceptSquash}
								{onAmend}
								{onSquash}
								{resetHeadCommit}
								isChained={idx != commits.length - 1}
								isHeadCommit={commit.id === headCommit?.id}
							/>
						</div>
					{/each}
				</div>
				{#if !readonly && type != 'integrated'}
					<div class="actions">
						{#if githubContext && !$pr$ && type == 'local'}
							<PushButton
								wide
								isLoading={isPushing || $prServiceState$?.busy}
								{projectId}
								{githubContext}
								on:trigger={async (e) => {
									try {
										await push({ createPr: e.detail.with_pr });
									} catch {
										toast.error('Failed to create pull qequest');
									}
								}}
							/>
						{:else if githubContext && !$pr$ && type == 'remote'}
							<Button
								wide
								kind="outlined"
								color="primary"
								loading={isPushing || $prServiceState$?.busy}
								on:click={async () => {
									try {
										await push({ createPr: true });
									} catch (e) {
										toast.error('Failed to create pull qequest');
									}
								}}
							>
								Create Pull Request
							</Button>
						{:else if type == 'local'}
							<Button
								kind="outlined"
								color="primary"
								loading={isPushing}
								on:click={async () => {
									try {
										await push();
									} catch {
										toast.error('Failed to push');
									}
								}}
							>
								{#if branch.requiresForce}
									Force Push
								{:else}
									Push
								{/if}
							</Button>
						{/if}
					</div>
				{/if}
			</div>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.wrapper {
		display: flex;
		flex-direction: column;
		border-top: 1px solid var(--clr-theme-container-outline-light);
	}
	.header {
		display: flex;
		padding: var(--space-16) var(--space-12) var(--space-16) var(--space-16);
		justify-content: space-between;
		gap: var(--space-8);
	}
	.title {
		display: flex;
		align-items: center;
		color: var(--clr-theme-scale-ntrl-0);
		gap: var(--space-8);
		overflow-x: hidden;
	}
	.content-wrapper {
		display: flex;
		flex-direction: column;
		padding: 0 var(--space-16) var(--space-20) var(--space-16);
		gap: var(--space-8);
	}
	.commits {
	}
	.actions {
		padding-left: var(--space-16);
	}
</style>
