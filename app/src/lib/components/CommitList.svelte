<script lang="ts">
	import CommitCard from './CommitCard.svelte';
	import CommitLines from './CommitLines.svelte';
	import { Project } from '$lib/backend/projects';
	import Dropzone from '$lib/components/Dropzone/Dropzone.svelte';
	import LineOverlay from '$lib/components/Dropzone/LineOverlay.svelte';
	import InsertEmptyCommitAction from '$lib/components/InsertEmptyCommitAction.svelte';
	import {
		ReorderDropzoneManagerFactory,
		type ReorderDropzone
	} from '$lib/dragging/reorderDropzoneManager';
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
	import { BaseBranch, Branch, Commit, type CommitStatus } from '$lib/vbranches/types';
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

	const reorderDropzoneManagerFactory = getContext(ReorderDropzoneManagerFactory);

	// Force the "base" commit lines to update when $branch updates.
	let tsKey: number | undefined;
	$: {
		$branch;
		tsKey = Date.now();
	}

	$: hasLocalColumn = $localCommits.length > 0;
	$: hasCommits = $branch.commits && $branch.commits.length > 0;
	$: headCommit = $branch.commits.at(0);
	$: hasLocalCommits = $localCommits.length > 0;
	$: hasUnknownCommits = $unknownCommits.length > 0;
	$: hasIntegratedCommits = $integratedCommits.length > 0;
	$: hasRemoteCommits = $remoteCommits.length > 0;
	$: hasShadowedCommits = $localCommits.some((c) => c.relatedTo);
	$: reorderDropzoneManager = reorderDropzoneManagerFactory.build($branch, [
		...$localCommits,
		...$remoteCommits
	]);

	$: forkPoint = $branch.forkPoint;
	$: upstreamForkPoint = $branch.upstreamData?.forkPoint;
	$: isRebased = !!forkPoint && !!upstreamForkPoint && forkPoint !== upstreamForkPoint;

	let baseIsUnfolded = false;

	function getOutType(commit: Commit): CommitStatus | undefined {
		if (!hasShadowedCommits) {
			if (!commit.next || commit.next.status === 'local') {
				return $unknownCommits.length > 0 ? 'upstream' : undefined;
			}
			return commit.next?.status;
		}

		let pointer: Commit | undefined = commit.next;

		while (pointer && !pointer.relatedTo) {
			pointer = pointer.next;
		}
		if (pointer) return pointer.status;
		return hasUnknownCommits ? 'upstream' : undefined;
	}

	function getBaseShadowOutType(): CommitStatus | undefined {
		if (!isRebased) return;
		if (hasIntegratedCommits) return 'integrated';
		if (hasShadowedCommits) return 'remote';
		if (hasUnknownCommits) return 'upstream';
	}

	function getBaseRemoteOutType(): CommitStatus | undefined {
		if (isRebased) return;
		if (hasIntegratedCommits) return 'integrated';
		if (hasShadowedCommits || hasRemoteCommits) return 'remote';
		if (hasUnknownCommits) return 'upstream';
	}

	function getInType(commit: Commit): CommitStatus | undefined {
		if (commit.prev) return getOutType(commit.prev || commit);
		if (commit.status === 'remote' || commit.relatedTo) return 'remote';
		if (commit.status === 'integrated') return 'integrated';
		if (commit) return getOutType(commit);
	}

	function insertBlankCommit(commitId: string, location: 'above' | 'below' = 'below') {
		if (!$branch || !$baseBranch) {
			console.error('Unable to insert commit');
			return;
		}
		branchController.insertBlankCommit($branch.id, commitId, location === 'above' ? -1 : 1);
	}

	function getReorderDropzoneOffset({
		isFirst = false,
		isMiddle = false,
		isLast = false
	}: {
		isFirst?: boolean;
		isMiddle?: boolean;
		isLast?: boolean;
	}) {
		if (isFirst) return 12;
		if (isMiddle) return 6;
		if (isLast) return 0;
		return 0;
	}
</script>

{#snippet reorderDropzone(dropzone: ReorderDropzone, yOffsetPx: number)}
	<Dropzone accepts={dropzone.accepts.bind(dropzone)} ondrop={dropzone.onDrop.bind(dropzone)}>
		{#snippet overlay({ hovered, activated })}
			<LineOverlay {hovered} {activated} {yOffsetPx} />
		{/snippet}
	</Dropzone>
{/snippet}

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
					first={idx === 0}
					last={idx === $unknownCommits.length - 1}
					commitUrl={$baseBranch?.commitUrl(commit.id)}
					isHeadCommit={commit.id === headCommit?.id}
				>
					<svelte:fragment slot="lines">
						<CommitLines
							{hasLocalColumn}
							{isRebased}
							localIn={'local'}
							localOut={'local'}
							author={commit.author}
							sectionFirst={idx === 0}
							inDashed={hasLocalColumn}
							outDashed={hasLocalColumn}
							commitStatus={commit.status}
							help={getAvatarTooltip(commit)}
							remoteIn={!isRebased ? 'upstream' : undefined}
							remoteOut={!isRebased && idx !== 0 ? 'upstream' : undefined}
							shadowIn={isRebased ? 'upstream' : undefined}
							shadowOut={idx !== 0 && isRebased ? 'upstream' : undefined}
						/>
					</svelte:fragment>
				</CommitCard>
			{/each}
		{/if}
		<InsertEmptyCommitAction isFirst on:click={() => insertBlankCommit($branch.head, 'above')} />
		<!-- LOCAL COMMITS -->
		{#if $localCommits.length > 0}
			{@render reorderDropzone(
				reorderDropzoneManager.topDropzone,
				getReorderDropzoneOffset({ isFirst: true })
			)}
			{#each $localCommits as commit, idx (commit.id)}
				<CommitCard
					{commit}
					{isUnapplied}
					type="local"
					first={idx === 0}
					branch={$branch}
					last={idx === $localCommits.length - 1}
					commitUrl={$baseBranch?.commitUrl(commit.id)}
					isHeadCommit={commit.id === headCommit?.id}
				>
					<svelte:fragment slot="lines">
						<CommitLines
							{isRebased}
							{hasLocalColumn}
							localIn={idx !== $localCommits.length - 1 ? 'local' : undefined}
							localOut={'local'}
							author={commit.author}
							sectionFirst={idx === 0}
							commitStatus={commit.status}
							help={getAvatarTooltip(commit)}
							shadowHelp={getAvatarTooltip(commit.relatedTo)}
							outDashed={hasLocalColumn && idx === 0}
							remoteIn={!isRebased ? getInType(commit) : undefined}
							remoteOut={!isRebased ? getOutType(commit) : undefined}
							shadowIn={isRebased ? getInType(commit) : undefined}
							shadowOut={isRebased ? getOutType(commit) : undefined}
							relatedToOther={commit?.relatedTo && commit.relatedTo.id !== commit.id}
							remoteRoot={idx === $localCommits.length - 1}
							last={idx === $localCommits.length - 1 && !hasRemoteCommits && !hasIntegratedCommits}
						/>
					</svelte:fragment>
				</CommitCard>

				{@render reorderDropzone(
					reorderDropzoneManager.dropzoneBelowCommit(commit.id),
					getReorderDropzoneOffset({
						isLast: $remoteCommits.length === 0 && idx + 1 === $localCommits.length,
						isMiddle: $remoteCommits.length > 0 && idx + 1 === $localCommits.length
					})
				)}

				<InsertEmptyCommitAction
					isLast={$remoteCommits.length === 0 && idx + 1 === $localCommits.length}
					isMiddle={$remoteCommits.length > 0 && idx + 1 === $localCommits.length}
					on:click={() => insertBlankCommit(commit.id, 'below')}
				/>
			{/each}
		{/if}
		<!-- REMOTE COMMITS -->
		{#if $remoteCommits.length > 0}
			{#each $remoteCommits as commit, idx (commit.id)}
				<CommitCard
					{commit}
					{isUnapplied}
					type="remote"
					first={idx === 0}
					branch={$branch}
					last={idx === $remoteCommits.length - 1}
					isHeadCommit={commit.id === headCommit?.id}
					commitUrl={$baseBranch?.commitUrl(commit.id)}
				>
					<svelte:fragment slot="lines">
						<CommitLines
							{hasLocalColumn}
							{isRebased}
							author={commit.author}
							sectionFirst={idx === 0}
							commitStatus={commit.status}
							help={getAvatarTooltip(commit)}
							shadowHelp={getAvatarTooltip(commit.relatedTo)}
							integrated={commit.isIntegrated}
							localRoot={idx === 0 && hasLocalCommits}
							outDashed={idx === 0 && commit.prev?.status === 'local'}
							remoteIn={!isRebased ? getInType(commit) : undefined}
							remoteOut={!isRebased ? getOutType(commit) : undefined}
							shadowIn={isRebased ? getInType(commit) : undefined}
							shadowOut={isRebased ? getOutType(commit) : undefined}
						/>
					</svelte:fragment>
				</CommitCard>
				{@render reorderDropzone(
					reorderDropzoneManager.dropzoneBelowCommit(commit.id),
					getReorderDropzoneOffset({
						isLast: idx + 1 === $remoteCommits.length
					})
				)}
				<InsertEmptyCommitAction
					isLast={idx + 1 === $remoteCommits.length}
					on:click={() => insertBlankCommit(commit.id, 'below')}
				/>
			{/each}
		{/if}
		<!-- INTEGRATED COMMITS -->
		{#if $integratedCommits.length > 0}
			{#each $integratedCommits as commit, idx (commit.id)}
				<CommitCard
					{commit}
					{isUnapplied}
					type="integrated"
					first={idx === 0}
					branch={$branch}
					isHeadCommit={commit.id === headCommit?.id}
					last={idx === $integratedCommits.length - 1}
					commitUrl={$baseBranch?.commitUrl(commit.id)}
				>
					<svelte:fragment slot="lines">
						<CommitLines
							{hasLocalColumn}
							{isRebased}
							author={commit.author}
							sectionFirst={idx === 0}
							commitStatus={commit.status}
							help={getAvatarTooltip(commit)}
							shadowIn={isRebased ? getInType(commit) : undefined}
							shadowOut={isRebased ? getOutType(commit) : undefined}
							remoteIn={!isRebased ? getInType(commit) : undefined}
							remoteOut={!isRebased ? getOutType(commit) : undefined}
							integrated={true}
							localRoot={idx === 0 && !hasRemoteCommits && hasLocalCommits}
						/>
					</svelte:fragment>
				</CommitCard>
			{/each}
		{/if}
		<!-- BASE -->
		<div class="base-row-container" class:base-row-container_unfolded={baseIsUnfolded}>
			<div
				class="base-row"
				tabindex="0"
				role="button"
				on:click|stopPropagation={() => (baseIsUnfolded = !baseIsUnfolded)}
				on:keydown={(e) => e.key === 'Enter' && (baseIsUnfolded = !baseIsUnfolded)}
			>
				<div class="base-row__lines">
					{#key tsKey}
						<CommitLines
							{hasLocalColumn}
							{isRebased}
							localRoot={!hasRemoteCommits && !hasIntegratedCommits && hasLocalCommits}
							shadowOut={getBaseShadowOutType()}
							remoteOut={getBaseRemoteOutType()}
							base
						/>
					{/key}
				</div>
				<div class="base-row__content">
					<span class="text-base-11 base-row__text"
						>Base commit <button
							class="base-row__commit-link"
							on:click={async () => await goto(`/${project.id}/base`)}
						>
							{$branch.forkPoint ? $branch.forkPoint.slice(0, 7) : ''}
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
		/* border-bottom: 1px solid var(--clr-border-2); */

		--base-top-margin: 8px;
		--base-icon-top: 16px;
		--base-unfolded: 48px;

		--avatar-first-top: 50px;
		--avatar-top: 16px;
	}

	/* .commit-group {
		padding-right: 14px;
		padding-left: 8px;
	} */

	/* BASE ROW */

	.base-row-container {
		display: flex;
		flex-direction: column;
		height: 20px;

		overflow: hidden;
		transition: height var(--transition-medium);

		&:hover {
			&:not(.base-row-container_unfolded) {
				height: 22px;
			}

			& .base-row {
				background-color: var(--clr-bg-2-muted);
			}
		}
	}

	.base-row-container_unfolded {
		height: var(--base-unfolded);
		--base-icon-top: 20px;

		& .base-row__text {
			opacity: 1;
		}
	}

	.base-row {
		display: flex;
		gap: 8px;
		border-top: 1px solid var(--clr-border-3);
		min-height: calc(var(--base-unfolded) - var(--base-top-margin));
		margin-top: var(--base-top-margin);
		transition: background-color var(--transition-fast);
	}

	.base-row__lines {
		display: flex;
		margin-top: -9px;
	}

	.base-row__content {
		display: flex;
		align-items: center;
	}

	.base-row__text {
		color: var(--clr-text-2);
		opacity: 0;
		margin-top: 2px;
		transition: opacity var(--transition-medium);
	}

	.base-row__commit-link {
		text-decoration: underline;
		cursor: pointer;
	}
</style>
