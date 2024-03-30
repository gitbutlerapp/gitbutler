<script lang="ts">
	import Button from '$lib/components/Button.svelte';
	import Link from '$lib/components/Link.svelte';
	import Tag from '$lib/components/Tag.svelte';
	import { getContext } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { marked } from 'marked';
	import type { PullRequest } from '$lib/github/types';

	export let pullrequest: PullRequest | undefined;

	const branchController = getContext(BranchController);
</script>

{#if pullrequest != undefined}
	<div class="wrapper">
		<div class="card">
			<div class="card__header text-base-body-14 text-semibold">
				<h2 class="text-base-14 text-semibold">
					{pullrequest.title}
					<span class="card__title-pr">
						<Link target="_blank" rel="noreferrer" href={pullrequest.htmlUrl}>
							#{pullrequest.number}
						</Link>
					</span>
				</h2>
				{#if pullrequest.draft}
					<Tag style="neutral" icon="draft-pr-small">Draft</Tag>
				{:else}
					<Tag style="success" kind="solid" icon="pr-small">Open</Tag>
				{/if}
			</div>

			<div class="card__content">
				<div class="text-base-13">
					<span class="text-bold">
						{pullrequest.author?.name}
					</span>
					wants to merge into
					<span class="code-line">
						{pullrequest.sourceBranch}
					</span>
					from
					<span class="code-line">
						{pullrequest.targetBranch}
					</span>
				</div>
				{#if pullrequest.body}
					<div class="markdown">
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
		gap: var(--size-16);
		max-width: 56rem;
	}
	.card__content {
		gap: var(--size-12);
	}
	.card__title-pr {
		opacity: 0.4;
		margin-left: var(--size-4);
	}
</style>
