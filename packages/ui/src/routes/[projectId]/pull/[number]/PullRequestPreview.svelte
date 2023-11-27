<script lang="ts">
	import Button from '$lib/components/Button.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { PullRequest } from '$lib/github/types';
	import { IconPullRequest, IconDraftPullRequest } from '$lib/icons';
	import Link from '$lib/components/Link.svelte';
	import Tooltip from '$lib/components/Tooltip.svelte';

	export let pullrequest: PullRequest | undefined;
	export let branchController: BranchController;
</script>

{#if pullrequest != undefined}
	<div class="flex w-full max-w-full flex-col gap-y-4 p-4">
		<div class="flex flex-grow flex-col gap-y-6">
			<div class="flex flex-col">
				<h class="truncate text-lg font-semibold">
					{pullrequest.title}
					<span class="text-color-4">
						<Link target="_blank" rel="noreferrer" href={pullrequest.htmlUrl} class="text-3">
							#{pullrequest.number}
						</Link>
					</span>
				</h>

				<div class="flex items-center gap-1 truncate">
					{#if pullrequest.draft}
						<div
							class="flex items-center gap-x-1 rounded-full bg-zinc-500 px-2 py-0.5 text-sm font-medium text-white"
						>
							<IconDraftPullRequest class="h-3.5 w-3.5 "></IconDraftPullRequest>

							<span>Draft</span>
						</div>
					{:else}
						<div
							class="flex items-center gap-x-1 rounded-full bg-green-500 px-2 py-0.5 text-sm font-medium text-white"
						>
							<IconPullRequest class="h-3.5 w-3.5  "></IconPullRequest>
							<span>Open</span>
						</div>
					{/if}
					<div class="text-color-3">
						<span class="font-semibold">
							{pullrequest.author?.name}
						</span>
						wants to merge into
						<span class="rounded bg-blue-500/10 px-1 py-0.5 text-blue-500">
							{pullrequest.sourceBranch}
						</span>
						from
						<span class="rounded bg-blue-500/10 px-1 py-0.5 text-blue-500">
							{pullrequest.targetBranch}
						</span>
					</div>
				</div>
			</div>

			{#if pullrequest.body}
				<div class="text-3">
					{@html pullrequest.body}
				</div>
			{/if}
		</div>
		<div class="w-1/2">
			<Tooltip label="Does not create a commit. Can be toggled.">
				<Button
					color="purple"
					height="small"
					on:click={() =>
						pullrequest &&
						branchController.createvBranchFromBranch(
							'refs/remotes/origin/' + pullrequest.targetBranch
						)}>Apply to working directory</Button
				>
			</Tooltip>
		</div>
	</div>
{/if}
