<script lang="ts">
	import CommitCard from './CommitCard.svelte';
	import CommitLines from './CommitLines.svelte';
	import { Project } from '$lib/backend/projects';
	import Button from '$lib/components/Button.svelte';
	import ReorderDropzone from '$lib/components/CommitList/ReorderDropzone.svelte';
	import QuickActionMenu from '$lib/components/QuickActionMenu.svelte';
	import { ReorderDropzoneIndexer } from '$lib/dragging/reorderDropzoneIndexer';
	import { getAvatarTooltip } from '$lib/utils/avatar';
	import { getContext } from '$lib/utils/context';
	import { getContextStore } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import {
		getIntegratedCommits,
		getLocalCommits,
		getRemoteCommits,
		getUnknownCommits
	} from '$lib/vbranches/contexts';
	import {
		BaseBranch,
		Branch,
		Commit,
		RemoteCommit,
		type CommitStatus
	} from '$lib/vbranches/types';
	import { goto } from '$app/navigation';

	export let isUnapplied: boolean;

	const branch = getContextStore(Branch);
	const localCommits = getLocalCommits();
	const remoteCommits = getRemoteCommits();
	const unknownCommits = getUnknownCommits();
	const integratedCommits = getIntegratedCommits();
	const baseBranch = getContextStore(BaseBranch);
	const project = getContext(Project);
	const branchController = getContext(BranchController);

	$: hasShadowColumn =
		$integratedCommits.length == 0 &&
		$remoteCommits.length == 0 &&
		$localCommits.length > 0 &&
		$localCommits.at(0)?.relatedTo &&
		$localCommits.at(0)?.relatedTo?.id != $localCommits.at(0)?.id;
	$: hasLocalColumn = $localCommits.length > 0;
	$: hasCommits = $branch.commits && $branch.commits.length > 0;
	$: headCommit = $branch.commits.at(0);
	$: hasLocalCommits = $localCommits.length > 0;
	$: hasUnknownCommits = $unknownCommits.length > 0;
	$: hasIntegratedCommits = $integratedCommits.length > 0;
	$: hasRemoteCommits = $remoteCommits.length > 0;
	$: hasShadowedCommits = $localCommits.some((c) => c.relatedTo);
	$: reorderDropzoneIndexer = new ReorderDropzoneIndexer([...$localCommits, ...$remoteCommits]);

	let baseIsUnfolded = false;

	function getRemoteOutType(commit: Commit | RemoteCommit): CommitStatus | undefined {
		let child = commit.children?.[0];
		while (child) {
			if (child.status == 'remote' || child.relatedTo) return 'remote';
			if (child.status == 'local') return 'remote';
			if (child.status == 'integrated') return 'integrated';
			child = child?.children?.[0];
		}
		if (hasUnknownCommits) return 'upstream';
	}

	function getRemoteInType(commit: Commit | RemoteCommit): CommitStatus | undefined {
		if (commit.status == 'local' && commit.relatedTo) return 'remote';
		if (commit.status == 'integrated') return 'integrated';
		let parent = commit.parent;
		if (parent?.status == 'remote') return 'remote';
		if (parent) return getRemoteInType(parent);
		if (hasUnknownCommits) return 'upstream';
		return 'remote';
	}

	function insertBlankCommit(commitId: string, location: 'above' | 'below' = 'below') {
		if (!$branch || !$baseBranch) {
			console.error('Unable to insert commit');
			return;
		}
		branchController.insertBlankCommit($branch.id, commitId, location == 'above' ? -1 : 1);
	}
</script>

{#if hasCommits || hasUnknownCommits}
	<div class="commits">
		<!-- UPSTREAM COMMITS -->
		{#if $unknownCommits.length > 0}
			{#each $unknownCommits as commit, idx (commit.id)}
				<CommitCard
					type="upstream"
					branch={$branch}
					{commit}
					{isUnapplied}
					first={idx == 0}
					last={idx == $unknownCommits.length - 1}
					commitUrl={$baseBranch?.commitUrl(commit.id)}
					isHeadCommit={commit.id === headCommit?.id}
				>
					<svelte:fragment slot="lines">
						<CommitLines
							{hasLocalColumn}
							{hasShadowColumn}
							localIn={'local'}
							localOut={'local'}
							author={commit.author}
							sectionFirst={idx == 0}
							inDashed={hasLocalColumn}
							outDashed={hasLocalColumn}
							commitStatus={commit.status}
							help={getAvatarTooltip(commit)}
							remoteIn={!hasShadowColumn ? 'upstream' : undefined}
							remoteOut={!hasShadowColumn && idx != 0 ? 'upstream' : undefined}
							shadowIn={hasShadowColumn ? getRemoteInType(commit) : undefined}
							shadowOut={idx != 0 && hasShadowColumn ? getRemoteOutType(commit) : undefined}
						/>
					</svelte:fragment>
				</CommitCard>
			{/each}
		{/if}
		<QuickActionMenu
			offset={$localCommits.length == 0 &&
			$remoteCommits.length == 0 &&
			$integratedCommits.length == 0
				? 0
				: 0.75}
			padding={1}
		>
			<Button style="ghost" size="tag" on:click={() => insertBlankCommit($branch.head, 'above')}
				>Insert blank commit</Button
			>
		</QuickActionMenu>
		<!-- LOCAL COMMITS -->
		{#if $localCommits.length > 0}
			<ReorderDropzone
				index={reorderDropzoneIndexer.topDropzoneIndex}
				indexer={reorderDropzoneIndexer}
			/>
			{#each $localCommits as commit, idx (commit.id)}
				<CommitCard
					{commit}
					{isUnapplied}
					type="local"
					first={idx == 0}
					branch={$branch}
					last={idx == $localCommits.length - 1}
					commitUrl={$baseBranch?.commitUrl(commit.id)}
					isHeadCommit={commit.id === headCommit?.id}
				>
					<svelte:fragment slot="lines">
						<CommitLines
							{hasLocalColumn}
							{hasShadowColumn}
							localIn={'local'}
							localOut={'local'}
							author={commit.author}
							sectionFirst={idx == 0}
							commitStatus={commit.status}
							help={getAvatarTooltip(commit)}
							outDashed={hasLocalColumn && idx == 0}
							sectionLast={idx == $localCommits.length - 1}
							remoteIn={!hasShadowColumn ? getRemoteInType(commit) : undefined}
							remoteOut={!hasShadowColumn ? getRemoteOutType(commit) : undefined}
							shadowIn={hasShadowColumn ? getRemoteInType(commit) : undefined}
							shadowOut={hasShadowColumn ? getRemoteOutType(commit) : undefined}
							relatedToOther={commit?.relatedTo && commit.relatedTo.id != commit.id}
							last={idx == $localCommits.length - 1 && !hasRemoteCommits && !hasIntegratedCommits}
						/>
					</svelte:fragment>
				</CommitCard>
				<ReorderDropzone
					index={reorderDropzoneIndexer.dropzoneIndexBelowCommit(commit.id)}
					indexer={reorderDropzoneIndexer}
				/>
				<QuickActionMenu
					padding={1}
					offset={$remoteCommits.length > 0 && idx + 1 == $localCommits.length ? 0.25 : 0}
				>
					<Button style="ghost" size="tag" on:click={() => insertBlankCommit(commit.id, 'below')}
						>Insert blank commit</Button
					>
				</QuickActionMenu>
			{/each}
		{/if}
		<!-- REMOTE COMMITS -->
		{#if $remoteCommits.length > 0}
			{#each $remoteCommits as commit, idx (commit.id)}
				<CommitCard
					{commit}
					{isUnapplied}
					type="remote"
					first={idx == 0}
					branch={$branch}
					last={idx == $remoteCommits.length - 1}
					isHeadCommit={commit.id == headCommit?.id}
					commitUrl={$baseBranch?.commitUrl(commit.id)}
				>
					<svelte:fragment slot="lines">
						<CommitLines
							{hasLocalColumn}
							{hasShadowColumn}
							author={commit.author}
							sectionFirst={idx == 0}
							commitStatus={commit.status}
							help={getAvatarTooltip(commit)}
							integrated={commit.isIntegrated}
							localRoot={idx == 0 && hasLocalCommits}
							outDashed={idx == 0 && commit.parent?.status == 'local'}
							remoteIn={!hasShadowColumn ? getRemoteInType(commit) : undefined}
							remoteOut={!hasShadowColumn ? getRemoteOutType(commit) : undefined}
							shadowIn={hasShadowColumn ? getRemoteInType(commit) : undefined}
							shadowOut={hasShadowColumn ? getRemoteOutType(commit) : undefined}
						/>
					</svelte:fragment>
				</CommitCard>
				<ReorderDropzone
					index={reorderDropzoneIndexer.dropzoneIndexBelowCommit(commit.id)}
					indexer={reorderDropzoneIndexer}
				/>
				<QuickActionMenu padding={1}>
					<Button style="ghost" size="tag" on:click={() => insertBlankCommit(commit.id, 'below')}
						>Insert blank commit</Button
					>
				</QuickActionMenu>
			{/each}
		{/if}
		<!-- INTEGRATED COMMITS -->
		{#if $integratedCommits.length > 0}
			{#each $integratedCommits as commit, idx (commit.id)}
				<CommitCard
					{commit}
					{isUnapplied}
					type="integrated"
					first={idx == 0}
					branch={$branch}
					isHeadCommit={commit.id === headCommit?.id}
					last={idx == $integratedCommits.length - 1}
					commitUrl={$baseBranch?.commitUrl(commit.id)}
				>
					<svelte:fragment slot="lines">
						<CommitLines
							{hasLocalColumn}
							{hasShadowColumn}
							author={commit.author}
							sectionFirst={idx == 0}
							commitStatus={commit.status}
							help={getAvatarTooltip(commit)}
							remoteIn={!hasShadowColumn ? getRemoteInType(commit) : undefined}
							remoteOut={!hasShadowColumn ? getRemoteOutType(commit) : undefined}
						/>
					</svelte:fragment>
				</CommitCard>
			{/each}
		{/if}
		<!-- BASE -->
		<div class="base-row-container" class:base-row-container_unfolded={baseIsUnfolded}>
			<div
				class="commit-group base-row"
				tabindex="0"
				role="button"
				on:click|stopPropagation={() => (baseIsUnfolded = !baseIsUnfolded)}
				on:keydown={(e) => e.key === 'Enter' && (baseIsUnfolded = !baseIsUnfolded)}
			>
				<div class="base-row__lines">
					<CommitLines
						{hasLocalColumn}
						{hasShadowColumn}
						localRoot={!hasRemoteCommits && !hasIntegratedCommits && hasLocalCommits}
						shadowOut={hasShadowedCommits ? 'remote' : 'upstream'}
						remoteOut={!hasShadowColumn && (hasIntegratedCommits || hasRemoteCommits)
							? 'remote'
							: !hasShadowColumn && hasUnknownCommits
								? 'upstream'
								: undefined}
						base
					/>
				</div>
				<div class="base-row__content">
					<span class="text-base-11 base-row__text"
						>Base commit <button
							class="base-row__commit-link"
							on:click={async () => await goto(`/${project.id}/base`)}
						>
							{$branch.mergeBase ? $branch.mergeBase.slice(0, 7) : ''}
						</button>
					</span>
				</div>
			</div>
		</div>
	</div>
{/if}

<style lang="postcss">
	.commits {
		display: flex;
		flex-direction: column;
		background-color: var(--clr-bg-2);
		border-top: 1px solid var(--clr-border-2);
		border-bottom: 1px solid var(--clr-border-2);

		--base-top-margin: var(--size-8);
		--base-icon-top: var(--size-16);
		--base-unfolded: var(--size-48);

		--avatar-first-top: 3.1rem;
		--avatar-top: var(--size-16);
	}

	.commit-group {
		/* padding-right: var(--size-14);
		padding-left: var(--size-8); */
	}

	/* BASE ROW */

	.base-row-container {
		display: flex;
		flex-direction: column;
		height: var(--size-20);

		overflow: hidden;
		transition: height var(--transition-medium);
	}

	.base-row-container_unfolded {
		height: var(--base-unfolded);
		--base-icon-top: var(--size-20);

		& .base-row__text {
			opacity: 1;
		}
	}

	.base-row {
		display: flex;
		gap: var(--size-8);
		border-top: 1px solid var(--clr-border-3);
		min-height: calc(var(--base-unfolded) - var(--base-top-margin));
		margin-top: var(--base-top-margin);
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-2-muted);
		}
	}

	.base-row__lines {
		display: flex;
		margin-top: calc(var(--size-8) * -1);
	}

	.base-row__content {
		display: flex;
		align-items: center;
	}

	.base-row__text {
		color: var(--clr-text-2);
		opacity: 0;
		margin-top: var(--size-2);
		transition: opacity var(--transition-medium);
	}

	.base-row__commit-link {
		text-decoration: underline;
		cursor: pointer;
	}
</style>
