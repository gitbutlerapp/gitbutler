<script lang="ts">
	import StackTip from '$components/v3/StackTip.svelte';
	import EmptyStack from '$lib/assets/illustrations/empty-stack-placeholder.svg?raw';
	import SelectACommit from '$lib/assets/illustrations/select-a-commit-preview.svg?raw';
	import CommitAndPush from '$lib/assets/illustrations/tip-commit-and-push.svg?raw';
	import ManageCommits from '$lib/assets/illustrations/tip-manage-commits.svg?raw';
	import WhatIsAStack from '$lib/assets/illustrations/tip-what-is-a-stack.svg?raw';
	import { previewModes, type PreviewMode } from '$components/v3/StackDetails.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
	import { slide } from 'svelte/transition';

	interface Props {
		mode: PreviewMode;
	}

	const { mode: initialMode }: Props = $props();

	let mode = $state<PreviewMode>(initialMode);

	$effect(() => {
		mode = initialMode;
	});

	const isTipViewingMode = $derived(mode === previewModes.ViewingTips);
	let activeTip = $state(0);

	function selectTip(tipIndex: number) {
		mode = previewModes.ViewingTips;
		activeTip = tipIndex;
	}
</script>

<div class="placeholder">
	<div class="top" class:white-bg={isTipViewingMode}>
		{#if mode === previewModes.EmptyBranch}
			<StackTip>
				{#snippet illustration()}
					{@html EmptyStack}
				{/snippet}
				{#snippet title()}
					This is a new stack
				{/snippet}
				{#snippet body()}
					Stack is a workflow for building branches sequentially to break features into smaller
					parts. You can also choose a regular single-branch flow.
				{/snippet}
			</StackTip>
		{/if}
		{#if mode === previewModes.SelectToPreview}
			<StackTip>
				{#snippet illustration()}
					{@html SelectACommit}
				{/snippet}
				{#snippet title()}
					Select a commit to preview
				{/snippet}
			</StackTip>
		{/if}
		{#if mode === previewModes.ViewingTips && activeTip === 0}
			<StackTip>
				{#snippet illustration()}
					{@html WhatIsAStack}
				{/snippet}
				{#snippet title()}
					What is a stack?
				{/snippet}
				{#snippet body()}
					<p>
						Stack is a workflow where branches are built sequentially, breaking large features into
						smaller parts. Each branch depends on the previous one. You can also choose a
						single-branch flow.
					</p>
				{/snippet}
			</StackTip>
		{/if}
		{#if mode === previewModes.ViewingTips && activeTip === 1}
			<StackTip>
				{#snippet illustration()}
					{@html CommitAndPush}
				{/snippet}
				{#snippet title()}
					Commit and push
				{/snippet}
				{#snippet body()}
					<p>
						File changes can be committed in any stack unless already committed in another, as they
						are dependent on a branch. When the dependent branch is selected, you can commit
						“locked” files. All branches in a stack are pushed together.
					</p>
				{/snippet}
			</StackTip>
		{/if}
		{#if mode === previewModes.ViewingTips && activeTip === 2}
			<StackTip>
				{#snippet illustration()}
					{@html ManageCommits}
				{/snippet}
				{#snippet title()}
					Manage commits
				{/snippet}
				{#snippet body()}
					<p>
						File changes can be committed in any stack unless already committed in another, as they
						are dependent on a branch. When the dependent branch is selected, you can commit
						“locked” files. All branches in a stack are pushed together.
					</p>
				{/snippet}
			</StackTip>
		{/if}
	</div>
	<div class="bottom">
		<div class="content">
			<div class="left">
				<div class="text-16 text-semibold">Tips</div>

				<button
					type="button"
					class={[
						'text-13 text-semibold',
						isTipViewingMode && activeTip === 0 ? 'text-clr2' : 'text-clr3'
					]}
					onclick={() => selectTip(0)}
				>
					{#if isTipViewingMode && activeTip === 0}
						<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
					{/if}
					What is a stack?
				</button>
				<button
					type="button"
					class={[
						'text-13 text-semibold',
						isTipViewingMode && activeTip === 1 ? 'text-clr2' : 'text-clr3'
					]}
					onclick={() => selectTip(1)}
				>
					{#if isTipViewingMode && activeTip === 1}
						<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
					{/if}
					Commit and push
				</button>
				<button
					type="button"
					class={[
						'text-13 text-semibold',
						isTipViewingMode && activeTip === 2 ? 'text-clr2' : 'text-clr3'
					]}
					onclick={() => selectTip(2)}
				>
					{#if isTipViewingMode && activeTip === 2}
						<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
					{/if}
					Manage commits
				</button>
			</div>
			<div class="right">
				<Link
					href="https://docs.gitbutler.com"
					target="_blank"
					underline={false}
					externalIcon={false}
					class="text-13 text-semibold text-clr2"><Icon name="doc" /> GitButler Docs</Link
				>
				<Link
					href="https://github.com/gitbutlerapp/gitbutler"
					target="_blank"
					underline={false}
					externalIcon={false}
					class="text-13 text-semibold text-clr2"><Icon name="github" /> Source Code</Link
				>
				<Link
					href="https://discord.com/invite/MmFkmaJ42D"
					target="_blank"
					underline={false}
					externalIcon={false}
					class="text-13 text-semibold text-clr2"><Icon name="discord" /> Join Community</Link
				>
			</div>
		</div>
	</div>
</div>

<style>
	.placeholder {
		flex: 1;
		position: relative;
		height: 100%;
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		justify-content: flex-start;
		overflow: hidden;
	}

	.top {
		flex: 1;
		width: 100%;
		display: flex;
		flex-direction: column;
		justify-content: center;

		gap: 24px;
		padding: 48px;

		&.white-bg {
			background-color: var(--clr-bg-1);
		}
	}

	.bottom {
		width: 100%;
		display: flex;
		padding: 48px;

		border-top: 1px solid var(--clr-border-2);

		& .left > div:first-child {
			margin-bottom: 20px;
		}

		& .right,
		& .left {
			flex: 0.5;
			display: flex;
			align-items: start;
			flex-direction: column;
			gap: 12px;
		}

		& .content {
			display: flex;
			width: 375px;
		}
	}

	.bottom .right {
		justify-content: end;
	}

	.active-page-indicator {
		content: '';
		position: absolute;
		left: 0;
		width: 12px;
		height: 18px;
		border-radius: var(--radius-m);
		background-color: var(--clr-core-ntrl-50);
		transform: translateX(-50%);
	}
</style>
