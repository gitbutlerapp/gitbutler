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

<a class="item" class:selected {href}>
	{#if branch.icon}
		<div class="item__icon"><Icon name={branch.icon} color={branch.color} /></div>
	{/if}
	<div class="item__info flex flex-col gap-2">
		<p class="text-base-13">
			{branch.displayName}
		</p>
		<div class="details">
			<span class="by-label text-base-11 details truncate">
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
	.item {
		display: flex;
		gap: var(--space-10);
		width: 100%;

		padding: var(--space-10) var(--space-8);
		border-radius: var(--radius-m);
	}

	.details {
		display: flex;
		align-items: center;
		gap: var(--space-6);
	}

	.by-label {
		flex: 1;
		color: var(--clr-theme-scale-ntrl-50);
	}

	.item:hover,
	.item:focus,
	.selected {
		background-color: var(--clr-theme-container-pale);
	}

	.item__icon {
		flex-shrink: 0;
	}

	.item__info {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		gap: var(--space-6);
	}
</style>
