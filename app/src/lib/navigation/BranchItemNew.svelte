<script lang="ts">
	import Icon from '$lib/shared/Icon.svelte';
	import TimeAgo from '$lib/shared/TimeAgo.svelte';
	import { stringToColor } from '@gitbutler/ui/utils/stringToColor';
	import { tooltip } from '@gitbutler/ui/utils/tooltip';
	import type { CombinedBranch } from '$lib/branches/types';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	interface Props {
		projectId: string;
		branch: CombinedBranch;
	}

	const { projectId, branch }: Props = $props();

	let href = $derived(getBranchLink(branch));
	let selected = $state(false);

	$effect(() => {
		selected = href ? $page.url.href.endsWith(href) : false;
		console.log(branch);
	});

	function getBranchLink(b: CombinedBranch): string | undefined {
		if (b.vbranch) return `/${projectId}/board/`;
		if (b.remoteBranch) return `/${projectId}/remote/${branch?.remoteBranch?.displayName}`;
		if (b.pr) return `/${projectId}/pull/${b.pr.number}`;
	}
</script>

<button
	class="branch"
	class:selected
	onmousedown={() => {
		if (href) goto(href);
	}}
>
	<h4 class="text-base-13 text-semibold branch-name">
		{branch.displayName}
	</h4>

	<div class="row">
		<div class="row-group">
			<div class="branch-authors">
				{#each branch.authors as author}
					<div
						use:tooltip={{
							text: author.name || 'Unknown',
							delay: 500
						}}
						class="author-avatar"
						style:background-color={stringToColor(author.name)}
						style:background-image="url({author.gravatarUrl})"
					></div>
				{/each}
			</div>
			<div class="branch-remotes">
				<!-- NEED API -->
				{#if branch.remoteBranch}
					<div class="branch-tag tag-remote">
						<span class="text-base-10 text-semibold">origin</span>
					</div>
					<!-- <div class="branch-tag tag-local">
						<span class="text-base-10 text-semibold">local</span>
					</div> -->
				{/if}
			</div>
		</div>

		<div class="row-group">
			{#if branch.pr}
				<div use:tooltip={{ text: branch.pr.title, delay: 500 }} class="branch-tag tag-pr">
					<span class="text-base-10 text-semibold">PR</span>
					<Icon name="pr-small" />
				</div>
			{/if}
			<!-- NEED API -->
			<!-- <div class="branch-tag tag-applied">
				<span class="text-base-10 text-semibold">applied</span>
			</div> -->
		</div>
	</div>

	<div class="row">
		<span class="branch-time text-base-11 details truncate">
			<TimeAgo date={branch.modifiedAt} />
			{#if branch.author?.name}
				by {branch.author?.name}
			{/if}
		</span>

		<!-- NEED API -->
		<div class="stats">
			<div use:tooltip={'Number of commits'} class="branch-tag tag-commits">
				<span class="text-base-10 text-semibold">34</span>
				<Icon name="commit" />
			</div>

			<div use:tooltip={'Code changes'} class="code-changes">
				<span class="text-base-10 text-semibold">+289</span>
				<span class="text-base-10 text-semibold">-129</span>
			</div>
		</div>
	</div>
</button>

<style lang="postcss">
	.branch {
		display: flex;
		flex-direction: column;
		padding: 10px 14px 12px 14px;
		gap: 8px;
		width: 100%;
		text-align: left;
		border-bottom: 1px solid var(--clr-border-3);
		transition: background-color var(--transition-fast);

		&:last-child {
			border-bottom: none;
		}
	}

	/* ROW */

	.row {
		display: flex;
		align-items: center;
		width: 100%;
		gap: 6px;
		justify-content: space-between;
	}

	.row-group {
		display: flex;
		align-items: center;
	}

	/* AUTHORS */

	.branch-authors {
		display: flex;
		margin-right: 6px;
	}

	.author-avatar {
		width: 16px;
		height: 16px;
		border-radius: 50%;
		margin-left: -4px;
		background-color: var(--clr-scale-ntrl-50);
		background-size: cover;

		&:first-child {
			margin-left: 0;
		}
	}

	/* TAG */

	.branch-tag {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 2px;
		padding: 0 4px;
		height: 16px;
		border-radius: var(--radius-s);
	}

	.tag-remote {
		background-color: var(--clr-theme-ntrl-soft-hover);
		color: var(--clr-text-1);
	}

	.tag-local {
		background-color: var(--clr-theme-ntrl-soft-hover);
		color: var(--clr-text-2);
	}

	.tag-pr {
		background-color: var(--clr-theme-succ-element);
		color: var(--clr-theme-succ-on-element);
	}

	.tag-applied {
		background-color: var(--clr-scale-ntrl-40);
		color: var(--clr-theme-ntrl-on-element);
		margin-left: 4px;

		&:first-child {
			margin-left: 0;
		}
	}

	.tag-commits {
		background-color: var(--clr-bg-3);
		color: var(--clr-text-2);
	}

	/*  */

	.code-changes {
		display: flex;
		height: 16px;

		& span {
			padding: 2px 4px;
			height: 100%;
		}

		& span:first-child {
			background-color: var(--clr-theme-succ-soft);
			color: var(--clr-theme-succ-on-soft);
			border-radius: var(--radius-s) 0 0 var(--radius-s);
		}

		& span:last-child {
			background-color: var(--clr-theme-err-soft);
			color: var(--clr-theme-err-on-soft);
			border-radius: 0 var(--radius-s) var(--radius-s) 0;
		}
	}

	.stats {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.branch-remotes {
		display: flex;
		gap: 2px;
	}

	.branch-name {
		width: 100%;
		white-space: nowrap;
		overflow-x: hidden;
		text-overflow: ellipsis;
	}

	.branch-time {
		color: var(--clr-scale-ntrl-50);
	}

	.branch:not(.selected):hover,
	.branch:not(.selected):focus {
		background-color: var(--clr-bg-1-muted);
		transition: none;
	}

	.selected {
		background-color: var(--clr-bg-2);
	}
</style>
