<script lang="ts">
	import BranchLabel from './BranchLabel.svelte';
	import { Project } from '$lib/backend/projects';
	import ViewPrContextMenu from '$lib/components/ViewPrContextMenu.svelte';
	import Button from '$lib/shared/Button.svelte';
	import Icon from '$lib/shared/Icon.svelte';
	import { getContext } from '$lib/utils/context';
	import { error } from '$lib/utils/toasts';
	import { tooltip } from '$lib/utils/tooltip';
	import { openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { mount, onDestroy, unmount } from 'svelte';
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
		if (popupMenu) unmount(popupMenu);
		return mount(ViewPrContextMenu, {
			target: document.body,
			props: { prUrl: copyablePrUrl }
		});
	}

	$: popupMenu = updateContextMenu(pr?.htmlUrl || '');

	onDestroy(() => {
		if (popupMenu) {
			unmount(popupMenu);
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
				<Button
					size="tag"
					icon="open-link"
					style="ghost"
					outline
					shrinkable
					on:click={(e) => {
						const url = base?.branchUrl(branch.name);
						if (url) openExternalUrl(url);
						e.preventDefault();
						e.stopPropagation();
					}}
				>
					{branch.displayName}
				</Button>
				{#if pr?.htmlUrl}
					<Button
						size="tag"
						clickable
						icon="pr-small"
						style="ghost"
						outline
						on:click={(e) => {
							const url = pr?.htmlUrl;
							if (url) openExternalUrl(url);
							e.preventDefault();
							e.stopPropagation();
						}}
					>
						View PR
					</Button>
				{/if}
			</div>
		</div>
		<div class="header__actions">
			<div class="header__buttons">
				<Button
					style="ghost"
					outline
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
							error(err);
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
	<div class="header__top-overlay" data-tauri-drag-region></div>
</div>

<style lang="postcss">
	.header__wrapper {
		z-index: var(--z-lifted);
		position: sticky;
		top: 12px;
	}
	.header {
		z-index: var(--z-lifted);
		position: relative;
		flex-direction: column;
		gap: 2px;
	}
	.header__top-overlay {
		z-index: var(--z-ground);
		position: absolute;
		top: -16px;
		left: 0;
		width: 100%;
		height: 20px;
		background: var(--clr-bg-2);
	}
	.header__info {
		display: flex;
		flex-direction: column;
		transition: margin var(--transition-slow);
		padding: 10px;
		gap: 10px;
		overflow: hidden;
	}
	.header__actions {
		display: flex;
		gap: 4px;
		background: var(--clr-bg-2);
		padding: 14px;
		justify-content: flex-end;
		border-radius: 0 0 var(--radius-m) var(--radius-m);
		user-select: none;
	}
	.header__buttons {
		display: flex;
		position: relative;
		gap: 4px;
	}

	.header__remote-branch {
		color: var(--clr-scale-ntrl-50);
		padding-left: 2px;
		padding-right: 2px;
		display: flex;
		gap: 4px;
		text-overflow: ellipsis;
		overflow-x: hidden;
		white-space: nowrap;
		align-items: center;
	}

	.status-tag {
		cursor: default;
		display: flex;
		align-items: center;
		gap: 2px;
		padding: 2px 6px 2px 4px;
		border-radius: var(--radius-m);
	}

	.remote {
		color: var(--clr-scale-ntrl-100);
		background: var(--clr-scale-ntrl-40);
	}
</style>
