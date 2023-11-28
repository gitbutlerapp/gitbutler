<script lang="ts" context="module">
	export type CommitType = 'local' | 'remote' | 'integrated';
</script>

<script lang="ts">
	import type { BaseBranch, Branch, Commit } from '$lib/vbranches/types';
	import type { CrossfadeParams, TransitionConfig } from 'svelte/transition';
	import PushButton from './PushButton.svelte';
	import type { GitHubIntegrationContext, PullRequest } from '$lib/github/types';
	import Button from '$lib/components/Button.svelte';
	import { flip } from 'svelte/animate';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { DraggableCommit, DraggableFile, DraggableHunk } from '$lib/draggables';
	import Icon from '$lib/icons/Icon.svelte';
	import CommitListItem from './CommitListItem.svelte';
	import Link from '$lib/components/Link.svelte';
	import Tag from './Tag.svelte';
	import { open } from '@tauri-apps/api/shell';
	import toast from 'svelte-french-toast';
	import { sleep } from '$lib/utils/sleep';

	export let branch: Branch;
	export let githubContext: GitHubIntegrationContext | undefined;
	export let base: BaseBranch | undefined | null;
	export let prPromise: Promise<PullRequest | undefined> | undefined;
	export let projectId: string;
	export let branchController: BranchController;
	export let type: CommitType;

	export let acceptAmend: (commit: Commit) => (data: any) => boolean;
	export let acceptSquash: (commit: Commit) => (data: any) => boolean;
	export let onAmend: (data: DraggableFile | DraggableHunk) => void;
	export let onSquash: (commit: Commit) => (data: DraggableCommit) => void;
	export let resetHeadCommit: () => void;
	export let createPr: () => Promise<PullRequest | undefined>;

	export let receive: (
		node: any,
		params: CrossfadeParams & {
			key: any;
		}
	) => () => TransitionConfig;

	export let send: (
		node: any,
		params: CrossfadeParams & {
			key: any;
		}
	) => () => TransitionConfig;

	let isPushing: boolean;

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

	async function push(opts?: { createPr: boolean }) {
		isPushing = true;
		await branchController.pushBranch(branch.id, branch.requiresForce);
		if (opts?.createPr) {
			await sleep(500);
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
						{#await prPromise then pr}
							{#if pr?.htmlUrl}
								<Tag
									icon="pr-small"
									color="neutral-light"
									clickable
									on:click={(e) => {
										console.log(pr?.htmlUrl);
										open(pr?.htmlUrl);
										e.preventDefault();
										e.stopPropagation();
									}}
								>
									PR
								</Tag>
							{/if}
						{/await}
					{/if}
				{:else if type == 'integrated'}
					Integrated
				{/if}
			</div>
			<div class="expander">
				<Icon name={expanded ? 'chevron-top' : 'chevron-down'} />
			</div>
		</button>
		{#if expanded}
			<div class="content-wrapper">
				<div class="commits">
					{#each commits as commit, idx (commit.id)}
						<div
							class="draggable-wrapper"
							in:receive={{ key: commit.id }}
							out:send={{ key: commit.id }}
							animate:flip
						>
							<CommitListItem
								{commit}
								{base}
								{projectId}
								isChained={idx != commits.length - 1}
								isHeadCommit={commit.id === headCommit?.id}
								{acceptAmend}
								{acceptSquash}
								{onAmend}
								{onSquash}
								{resetHeadCommit}
							/>
						</div>
					{/each}
				</div>
				{#if type != 'integrated'}
					<div class="actions">
						{#await prPromise then pr}
							{#if githubContext && !pr && type == 'local'}
								<PushButton
									wide
									isLoading={isPushing}
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
							{:else if githubContext && !pr && type == 'remote'}
								<Button
									wide
									kind="outlined"
									color="primary"
									id="push-commits"
									loading={isPushing}
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
									id="push-commits"
									loading={isPushing}
									on:click={() => push()}
								>
									{#if branch.requiresForce}
										Force Push
									{:else}
										Push
									{/if}
								</Button>
							{/if}
						{/await}
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
