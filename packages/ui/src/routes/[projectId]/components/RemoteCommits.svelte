<script lang="ts">
	import IconButton from '$lib/components/IconButton.svelte';
	import Link from '$lib/components/Link.svelte';
	import Tooltip from '$lib/components/Tooltip.svelte';
	import type { DraggableCommit, DraggableFile, DraggableHunk } from '$lib/draggables';
	import type { GitHubIntegrationContext, PullRequest } from '$lib/github/types';
	import { IconGithub } from '$lib/icons';
	import { dropzone } from '$lib/utils/draggable';
	import type { BaseBranch, Branch, Commit } from '$lib/vbranches/types';
	import { flip } from 'svelte/animate';
	import type { CrossfadeParams, TransitionConfig } from 'svelte/transition';
	import CommitCard from './CommitCard.svelte';

	export let branch: Branch;
	export let githubContext: GitHubIntegrationContext | undefined;
	export let base: BaseBranch | undefined | null;
	export let prPromise: Promise<PullRequest | undefined> | undefined;
	export let projectId: string;

	export let acceptAmend: (commit: Commit) => (data: any) => boolean;
	export let acceptSquash: (commit: Commit) => (data: any) => boolean;
	export let onAmend: (data: DraggableFile | DraggableHunk) => void;
	export let onSquash: (commit: Commit) => (data: DraggableCommit) => void;
	export let resetHeadCommit: () => void;
	export let createPr: () => void;

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

	$: remoteCommits = branch.commits.filter((c) => c.isRemote && !c.isIntegrated);
	$: headCommit = branch.commits[0];

	function branchUrl(target: BaseBranch | undefined | null, upstreamBranchName: string) {
		if (!target) return undefined;
		const baseBranchName = target.branchName.split('/')[1];
		const parts = upstreamBranchName.split('/');
		const branchName = parts[parts.length - 1];
		return `${target.repoBaseUrl}/compare/${baseBranchName}...${branchName}`;
	}
</script>

{#if remoteCommits.length > 0}
	<div class="relative">
		<div
			class="dark:form-dark-600 absolute top-4 ml-[0.75rem] w-px bg-gradient-to-b from-light-600 via-light-600 via-90% dark:from-dark-400 dark:via-dark-400"
			style="height: calc(100% - 1rem);"
		/>

		<div class="relative flex flex-grow flex-col gap-2">
			<div
				class="dark:form-dark-600 absolute top-4 ml-[0.75rem] h-px w-6 bg-gradient-to-r from-light-600 via-light-600 via-10% dark:from-dark-400 dark:via-dark-400"
			/>

			<div class="relative max-w-full flex-grow overflow-hidden py-2 pl-12 pr-2 font-mono text-sm">
				{#if branch.upstream}
					<div class="flex gap-2">
						<Link
							target="_blank"
							rel="noreferrer"
							href={branchUrl(base, branch.upstream?.name)}
							class="inline-block max-w-full truncate text-sm font-bold"
						>
							{branch.upstream.name.split('refs/remotes/')[1]}
						</Link>
						{#await prPromise then pr}
							{#if githubContext && pr}
								<a target="_blank" rel="noreferrer" href={pr.htmlUrl}>
									<Tooltip label="&nbsp; Go to Pull Request &nbsp;" placement="right">
										<IconGithub class="text-color-5 h-4 w-4"></IconGithub>
									</Tooltip>
								</a>
							{:else if githubContext}
								<button class="text-color-4" on:click={createPr}>
									<Tooltip label="&nbsp; Create Pull Request &nbsp;" placement="right">
										<IconGithub class="h-4 w-4"></IconGithub>
									</Tooltip>
								</button>
							{/if}
						{/await}
					</div>
				{/if}
			</div>

			{#each remoteCommits as commit (commit.id)}
				<div
					class="flex w-full items-center gap-x-2 pb-2 pr-4"
					in:receive={{ key: commit.id }}
					out:send={{ key: commit.id }}
					animate:flip
				>
					{#if commit.id === headCommit?.id}
						<div class="group relative ml-[0.4rem] mr-1.5 h-3 w-3" title="Reset this commit">
							<div
								class="insert-0 absolute h-3 w-3 rounded-full border-2 border-light-600 bg-light-600 group-hover:opacity-0 dark:border-dark-400 dark:bg-dark-400"
								class:bg-light-500={commit.isRemote}
								class:dark:bg-dark-500={commit.isRemote}
							/>
							<IconButton
								class="insert-0 absolute opacity-0 group-hover:opacity-100"
								icon="question-mark"
								on:click={resetHeadCommit}
							/>
						</div>
					{:else}
						<div class="ml-[0.4rem] mr-1.5">
							<div
								class="h-3 w-3 rounded-full border-2 border-light-600 bg-light-600 dark:border-dark-400 dark:bg-dark-400"
								class:bg-light-500={commit.isRemote}
								class:dark:bg-dark-500={commit.isRemote}
							/>
						</div>
					{/if}

					<div
						class="relative h-full flex-grow overflow-hidden"
						use:dropzone={{
							active: 'amend-dz-active',
							hover: 'amend-dz-hover',
							accepts: acceptAmend(commit),
							onDrop: onAmend
						}}
						use:dropzone={{
							active: 'squash-dz-active',
							hover: 'squash-dz-hover',
							accepts: acceptSquash(commit),
							onDrop: onSquash(commit)
						}}
					>
						<div
							class="amend-dz-marker absolute z-10 hidden h-full w-full items-center justify-center rounded bg-blue-100/70 outline-dashed outline-2 -outline-offset-8 outline-light-600 dark:bg-blue-900/60 dark:outline-dark-300"
						>
							<div class="hover-text font-semibold">Amend</div>
						</div>
						<div
							class="squash-dz-marker absolute z-10 hidden h-full w-full items-center justify-center rounded bg-blue-100/70 outline-dashed outline-2 -outline-offset-8 outline-light-600 dark:bg-blue-900/60 dark:outline-dark-300"
						>
							<div class="hover-text font-semibold">Squash</div>
						</div>

						<CommitCard {commit} {projectId} commitUrl={base?.commitUrl(commit.id)} />
					</div>
				</div>
			{/each}
		</div>
	</div>
{/if}
