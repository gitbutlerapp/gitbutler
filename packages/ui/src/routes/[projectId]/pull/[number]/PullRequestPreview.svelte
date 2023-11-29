<script lang="ts">
	import Button from '$lib/components/Button.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { PullRequest } from '$lib/github/types';
	import Link from '$lib/components/Link.svelte';
	import Tooltip from '$lib/components/Tooltip.svelte';
	import Tag from '../../components/Tag.svelte';
	import TextArea from '$lib/components/TextArea.svelte';

	export let pullrequest: PullRequest | undefined;
	export let branchController: BranchController;
</script>

{#if pullrequest != undefined}
	<div class="wrapper max-w-4xl">
		<div class="pr">
			<div class="text-base-body-16 text-semibold flex items-center justify-between">
				<span class="whitespace-pre-wrap">
					{pullrequest.title}
					<span class="text-color-4">
						<Link target="_blank" rel="noreferrer" href={pullrequest.htmlUrl} class="text-3">
							#{pullrequest.number}
						</Link>
					</span>
				</span>
				{#if pullrequest.draft}
					<Tag color="neutral-dim" icon="pr-draft">Draft</Tag>
				{:else}
					<Tag color="success" icon="pr-draft" filled>Open</Tag>
				{/if}
			</div>

			<div class="flex items-center gap-1">
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
			{#if pullrequest.body}
				<div class="body">
					{@html pullrequest.body}
				</div>
			{/if}
			<div class="actions">
				<Tooltip label="Does not create a commit. Can be toggled.">
					<Button
						color="primary"
						on:click={() =>
							pullrequest &&
							branchController.createvBranchFromBranch(
								'refs/remotes/origin/' + pullrequest.targetBranch
							)}>Apply</Button
					>
				</Tooltip>
			</div>
		</div>
	</div>
{/if}

<style lang="postcss">
	.wrapper {
		display: flex;
		flex-direction: column;
		gap: var(--space-16);
	}
	.actions {
		display: flex;
		justify-content: space-between;
		align-items: flex-end;
	}
	.pr {
		display: flex;
		flex-direction: column;
		border: 1px solid var(--clr-theme-container-outline-light);
		background-color: var(--clr-theme-container-light);
		padding: var(--space-16);
		border-radius: var(--radius-m);
		gap: var(--space-12);
	}
	.body {
		white-space: wrap;
	}
</style>
