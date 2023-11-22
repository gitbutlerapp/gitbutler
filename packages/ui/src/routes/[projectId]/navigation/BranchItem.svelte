<script lang="ts">
	import AuthorIcons from '$lib/components/AuthorIcons.svelte';
	import TimeAgo from '$lib/components/TimeAgo.svelte';
	import Icon from '$lib/icons/Icon.svelte';
	import type { CombinedBranch } from '$lib/branches/types';

	export let projectId: string;
	export let branch: CombinedBranch;

	$: href = branch.pr
		? `/${projectId}/pull/${branch.pr.number}`
		: `/${projectId}/remote/${branch?.branch?.sha}`;
</script>

<a class="item" {href}>
	<div class="item__icon"><Icon name={branch.icon} color="pop" /></div>
	<div class="item__info flex flex-col gap-2 overflow-hidden">
		<p class="text-base-13 truncate">
			{branch.displayName}
		</p>
		<p
			class="text-base-11 flex w-full justify-between"
			style="color: var(--clr-theme-scale-ntrl-50)"
		>
			<TimeAgo date={branch.createdAt} />
			by {branch.author?.name ?? 'unknown'}
			<AuthorIcons authors={branch.authors} />
		</p>
	</div>
</a>

<style>
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
	.item:focus {
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
