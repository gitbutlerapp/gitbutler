<script lang="ts">
	import { page } from '$app/stores';
	import AuthorIcons from '$lib/components/AuthorIcons.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import Icon from '$lib/icons/Icon.svelte';
	import type { CombinedBranch } from '$lib/branches/types';

	export let projectId: string;
	export let branch: CombinedBranch;

	function getBranchLink(b: CombinedBranch): string | undefined {
		if (b.pr) return `/${projectId}/pull/${b.pr.number}`;
		if (b.vbranch?.active) return `/${projectId}/board/`;
		if (b.vbranch) return `/${projectId}/stashed/${b.vbranch.id}`;
		if (b.remoteBranch) return `/${projectId}/remote/${branch?.remoteBranch?.sha}`;
	}

	$: href = getBranchLink(branch);
	$: selected = href ? $page.url.href.includes(href) : false;
</script>

<a class="branch" class:selected {href}>
	{#if branch.icon}
		<div class="item__icon"><Icon name={branch.icon} color={branch.color} /></div>
	{/if}
	<div class="branch__info flex flex-col gap-2">
		<p class="text-base-body-13 branch__name">
			{branch.displayName}
		</p>
		<div class="branch__details">
			<span class="branch__author text-base-body-11 details truncate">
				<TimeAgo date={branch.modifiedAt} />
				{#if branch.author}
					by {branch.author?.name ?? 'unknown'}
				{/if}
			</span>
			<AuthorIcons authors={branch.authors} />
		</div>
	</div>
</a>

<style lang="postcss">
	.branch {
		display: flex;
		gap: var(--space-10);
		width: 100%;

		padding: var(--space-10) var(--space-8);
		border-radius: var(--radius-m);
	}

	.branch__info {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		overflow-x: hidden;
		gap: var(--space-6);
	}

	.branch__details {
		display: flex;
		align-items: center;
		gap: var(--space-6);
	}

	.branch__name {
		overflow-x: hidden;
		text-overflow: ellipsis;
	}

	.branch__author {
		flex: 1;
		color: var(--clr-theme-scale-ntrl-50);
	}

	.branch:hover,
	.branch:focus,
	.selected {
		background-color: var(--clr-theme-container-pale);
	}

	.item__icon {
		flex-shrink: 0;
	}
</style>
