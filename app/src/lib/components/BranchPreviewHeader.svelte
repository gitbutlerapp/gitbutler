<script lang="ts">
	import BranchLabel from './BranchLabel.svelte';
	import Tag from './Tag.svelte';
	import { Project } from '$lib/backend/projects';
	import Button from '$lib/components/Button.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import ViewPrContextMenu from '$lib/components/ViewPrContextMenu.svelte';
	import { getContext } from '$lib/utils/context';
	import { tooltip } from '$lib/utils/tooltip';
	import { openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { onDestroy } from 'svelte';
	import toast from 'svelte-french-toast';
	import type { PullRequest } from '$lib/github/types';
	import type { BaseBranch, RemoteBranch } from '$lib/vbranches/types';
	import { goto } from '$app/navigation';

	export let branch: RemoteBranch;
	export let base: BaseBranch | undefined | null;
	export let pr: PullRequest | undefined;

	const branchController = getContext(BranchController);
	const project = getContext(Project);

	let isApplying = false;

	function updateContextMenu(copyablePrUrl: string) {
		if (popupMenu) popupMenu.$destroy();
		return new ViewPrContextMenu({
			target: document.body,
			props: { prUrl: copyablePrUrl }
		});
	}

	$: popupMenu = updateContextMenu(pr?.htmlUrl || '');

	onDestroy(() => {
		if (popupMenu) {
			popupMenu.$destroy();
		}
	});
</script>

<div class="header__wrapper">
	<div class="header card">
		<div class="header__info">
			<BranchLabel disabled bind:name={branch.name} />
			<div class="header__remote-branch">
				<div
					class="status-tag text-base-11 text-semibold remote"
					use:tooltip={'At least some of your changes have been pushed'}
				>
					<Icon name="remote-branch-small" /> remote
				</div>
				<Tag
					icon="open-link"
					style="ghost"
					kind="solid"
					clickable
					shrinkable
					on:click={(e) => {
						const url = base?.branchUrl(branch.name);
						if (url) openExternalUrl(url);
						e.preventDefault();
						e.stopPropagation();
					}}
				>
					origin/{branch.displayName}
				</Tag>
				{#if pr?.htmlUrl}
					<Tag
						icon="pr-small"
						style="ghost"
						kind="solid"
						clickable
						on:click={(e) => {
							const url = pr?.htmlUrl;
							if (url) openExternalUrl(url);
							e.preventDefault();
							e.stopPropagation();
						}}
					>
						View PR
					</Tag>
				{/if}
			</div>
		</div>
		<div class="header__actions">
			<div class="header__buttons">
				<Button
					style="ghost"
					kind="solid"
					help="Restores these changes into your working directory"
					icon="plus-small"
					loading={isApplying}
					on:click={async () => {
						isApplying = true;
						try {
							await branchController.createvBranchFromBranch(branch.name);
							goto(`/${project.id}/board`);
						} catch (e) {
							const err = 'Failed to apply branch';
							toast.error(err);
							console.error(err, e);
						} finally {
							isApplying = false;
						}
					}}
				>
					Apply
				</Button>
			</div>
		</div>
	</div>
	<div class="header__top-overlay" data-tauri-drag-region />
</div>

<style lang="postcss">
	.header__wrapper {
		z-index: var(--z-lifted);
		position: sticky;
		top: var(--size-12);
	}
	.header {
		z-index: var(--z-lifted);
		position: relative;
		flex-direction: column;
		gap: var(--size-2);
	}
	.header__top-overlay {
		z-index: var(--z-ground);
		position: absolute;
		top: calc(var(--size-16) * -1);
		left: 0;
		width: 100%;
		height: var(--size-20);
		background: var(--clr-bg-2);
	}
	.header__info {
		display: flex;
		flex-direction: column;
		transition: margin var(--transition-slow);
		padding: var(--size-10);
		gap: var(--size-10);
		overflow: hidden;
	}
	.header__actions {
		display: flex;
		gap: var(--size-4);
		background: var(--clr-bg-2);
		padding: var(--size-14);
		justify-content: flex-end;
		border-radius: 0 0 var(--radius-m) var(--radius-m);
		user-select: none;
	}
	.header__buttons {
		display: flex;
		position: relative;
		gap: var(--size-4);
	}

	.header__remote-branch {
		color: var(--clr-scale-ntrl-50);
		padding-left: var(--size-2);
		padding-right: var(--size-2);
		display: flex;
		gap: var(--size-4);
		text-overflow: ellipsis;
		overflow-x: hidden;
		white-space: nowrap;
		align-items: center;
	}

	.status-tag {
		cursor: default;
		display: flex;
		align-items: center;
		gap: var(--size-2);
		padding: var(--size-2) var(--size-6) var(--size-2) var(--size-4);
		border-radius: var(--radius-m);
	}

	.remote {
		color: var(--clr-scale-ntrl-100);
		background: var(--clr-scale-ntrl-40);
	}
</style>
