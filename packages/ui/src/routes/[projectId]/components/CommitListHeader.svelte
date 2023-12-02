<script lang="ts">
	import type { PullRequest } from '$lib/github/types';
	import type { BaseBranch, Branch } from '$lib/vbranches/types';
	import type { Observable } from 'rxjs';
	import { branchUrl, type CommitType } from './commitList';
	import { open } from '@tauri-apps/api/shell';
	import Link from '$lib/components/Link.svelte';
	import Icon from '$lib/icons/Icon.svelte';
	import Tag from './Tag.svelte';
	import { onMount } from 'svelte';

	export let branch: Branch;
	export let expanded: boolean;
	export let pr$: Observable<PullRequest | undefined> | undefined;
	export let type: CommitType;
	export let base: BaseBranch | undefined | null;
	export let height: number | undefined;

	let element: HTMLButtonElement | undefined = undefined;

	onMount(() => (height = element?.offsetHeight));
</script>

<button class="header" bind:this={element} on:click={() => (expanded = !expanded)}>
	<div class="title text-base-12 text-bold">
		{#if type == 'local'}
			Local
		{:else if type == 'remote'}
			{#if branch.upstream}
				<Link
					target="_blank"
					rel="noreferrer"
					href={branchUrl(base, branch.upstream?.name)}
					class="inline-block max-w-full truncate"
				>
					{branch.upstream.name.split('refs/remotes/')[1]}
				</Link>
				{#if $pr$?.htmlUrl}
					<Tag
						icon="pr-small"
						color="neutral-light"
						clickable
						on:click={(e) => {
							const url = $pr$?.htmlUrl;
							if (url) open(url);
							e.preventDefault();
							e.stopPropagation();
						}}
					>
						PR
					</Tag>
				{/if}
			{/if}
		{:else if type == 'integrated'}
			Integrated
		{/if}
	</div>
	<div class="expander">
		<Icon name={expanded ? 'chevron-down' : 'chevron-top'} />
	</div>
</button>

<style lang="postcss">
	.header {
		display: flex;
		padding: var(--space-16) var(--space-12) var(--space-16) var(--space-16);
		justify-content: space-between;
		gap: var(--space-8);
	}
	.title {
		display: flex;
		align-items: center;
		color: var(--clr-theme-scale-ntrl-0);
		gap: var(--space-8);
		overflow-x: hidden;
	}
</style>
