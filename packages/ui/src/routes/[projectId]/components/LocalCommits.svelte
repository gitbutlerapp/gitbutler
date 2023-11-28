<script lang="ts">
	import type { BaseBranch, Branch, Commit } from '$lib/vbranches/types';
	import { slide, type CrossfadeParams, type TransitionConfig } from 'svelte/transition';
	import PushButton from './PushButton.svelte';
	import type { GitHubIntegrationContext, PullRequest } from '$lib/github/types';
	import Button from '$lib/components/Button.svelte';
	import IconButton from '$lib/components/IconButton.svelte';
	import { flip } from 'svelte/animate';
	import CommitCard from './CommitCard.svelte';
	import { dropzone } from '$lib/utils/draggable';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { DraggableCommit, DraggableFile, DraggableHunk } from '$lib/draggables';

	export let branch: Branch;
	export let githubContext: GitHubIntegrationContext | undefined;
	export let base: BaseBranch | undefined | null;
	export let prPromise: Promise<PullRequest | undefined> | undefined;
	export let projectId: string;
	export let branchController: BranchController;

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

	let isPushing: boolean;

	$: headCommit = branch.commits[0];
	$: localCommits = branch.commits.filter((c) => !c.isIntegrated && !c.isRemote);

	async function push() {
		isPushing = true;
		await branchController.pushBranch(branch.id, branch.requiresForce);
		isPushing = false;
	}
	/* test commit */
</script>

{#if localCommits.length > 0 || (branch.upstream && branch.upstream.commits.length > 0)}
	<div class="relative" transition:slide={{ duration: 150 }}>
		<div
			class="dark:form-dark-600 absolute top-4 ml-[0.75rem] w-px bg-gradient-to-b from-light-400 via-light-500 via-90% pt-4 dark:from-dark-600 dark:via-dark-600"
			style="height: calc(100% - 1rem);"
		/>

		<div class="relative flex flex-col gap-2">
			<div
				class="dark:form-dark-600 absolute top-4 ml-[0.75rem] h-px w-6 bg-gradient-to-r from-light-400 via-light-400 via-10% dark:from-dark-600 dark:via-dark-600"
			/>
			<div class="relative ml-10 mr-6 flex justify-end py-2">
				<div class="ml-2 flex-grow font-mono text-sm font-bold text-dark-300 dark:text-light-300">
					local
				</div>
				{#if githubContext && !prPromise}
					<PushButton
						isLoading={isPushing}
						{projectId}
						{githubContext}
						on:trigger={(e) => {
							push()?.finally(() => {
								if (e.detail.with_pr) createPr();
							});
						}}
					/>
				{:else}
					<Button
						kind="outlined"
						color="primary"
						id="push-commits"
						loading={isPushing}
						on:click={push}
					>
						{#if branch.requiresForce}
							<span class="purple">Force Push</span>
						{:else}
							<span class="purple">Push</span>
						{/if}
					</Button>
				{/if}
			</div>

			{#each localCommits as commit (commit.id)}
				<div
					class="flex w-full items-center gap-x-2 pb-2 pr-4"
					in:receive={{ key: commit.id }}
					out:send={{ key: commit.id }}
					animate:flip
				>
					{#if commit.id === headCommit?.id}
						<div class="group relative ml-[0.4rem] mr-1.5 h-3 w-3" title="Reset this commit">
							<div
								class="insert-0 border-color-4 bg-color-3 absolute h-3 w-3 rounded-full border-2 transition-opacity group-hover:opacity-0"
							/>
							<IconButton
								class="insert-0 absolute opacity-0 group-hover:opacity-100"
								icon="question-mark"
								on:click={resetHeadCommit}
							/>
						</div>
					{:else}
						<div class="ml-[0.4rem] mr-1.5">
							<div class="border-color-4 h-3 w-3 rounded-full border-2" />
						</div>
					{/if}
					<div
						class="relative h-full flex-grow overflow-hidden px-2"
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
