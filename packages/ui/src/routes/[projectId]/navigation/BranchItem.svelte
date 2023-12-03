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
	<div class="item__icon"><Icon name={branch.icon} color={branch.color} /></div>
	<div class="item__info flex flex-col gap-2 overflow-hidden">
		<p class="text-base-13 truncate">
			{branch.displayName}
		</p>
		<p
			class="text-base-11 flex w-full justify-between"
			style="color: var(--clr-theme-scale-ntrl-50)"
		>
			<TimeAgo date={branch.modifiedAt} />
			{#if branch.author}
				by {branch.author?.name ?? 'unknown'}
			{/if}
			<AuthorIcons authors={branch.authors} />
		</p>
	</div>
</a>

<style lang="postcss">
	.item {
		display: flex;
		gap: var(--space-10);
		width: 100%;

		padding-top: var(--space-10);
		padding-bottom: var(--space-10);
		padding-left: var(--space-8);
		padding-right: var(--space-8);
		border-radius: var(--radius-m);
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
