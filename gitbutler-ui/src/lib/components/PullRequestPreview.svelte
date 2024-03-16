<script lang="ts">
	import Button from '$lib/components/Button.svelte';
	import Link from '$lib/components/Link.svelte';
	import Tag from '$lib/components/Tag.svelte';
	import { getContextByClass } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { marked } from 'marked';
	import type { PullRequest } from '$lib/github/types';

	export let pullrequest: PullRequest | undefined;
	const branchController = getContextByClass(BranchController);
</script>

{#if pullrequest != undefined}
	<div class="wrapper max-w-4xl">
		<div class="pr card">
			<div class="card__header text-base-body-14 text-semibold">
				<span class="card__title whitespace-pre-wrap">
					{pullrequest.title}
					<span class="text-color-4">
						<Link target="_blank" rel="noreferrer" href={pullrequest.htmlUrl} class="text-3">
							#{pullrequest.number}
						</Link>
					</span>
				</span>
				{#if pullrequest.draft}
					<Tag color="light" icon="pr-draft">Draft</Tag>
				{:else}
					<Tag color="success" icon="pr-draft" filled>Open</Tag>
				{/if}
			</div>

			<div class="card__content">
				<div class="text-base-13">
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
				{#if pullrequest.body}
					<div class="body">
						{@html marked.parse(pullrequest.body)}
					</div>
				{/if}
			</div>
			<div class="card__footer">
				<Button
					help="Does not create a commit. Can be toggled."
					color="primary"
					on:click={() =>
						pullrequest &&
						branchController.createvBranchFromBranch(
							'refs/remotes/origin/' + pullrequest.targetBranch
						)}>Apply</Button
				>
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
	.card__content {
		gap: var(--space-12);
	}
	.body {
		white-space: wrap;
	}
</style>
