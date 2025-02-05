<script lang="ts">
	import { previewModes, type PreviewMode } from '$components/StackDetails.svelte';
	import StackContentPlaceholderEmptyBranch from '$components/v3/StackContentPlaceholder_EmptyBranch.svelte';
	import StackContentPlaceholderSelectToPreview from '$components/v3/StackContentPlaceholder_SelectToPreview.svelte';
	import StackContentTip0 from '$components/v3/StackContentTip_0.svelte';
	import StackContentTip1 from '$components/v3/StackContentTip_1.svelte';
	import StackContentTip2 from '$components/v3/StackContentTip_2.svelte';
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
	<div class="placeholder__top" class:white-bg={isTipViewingMode}>
		{#if mode === previewModes.EmptyBranch}
			<StackContentPlaceholderEmptyBranch />
		{/if}
		{#if mode === previewModes.SelectToPreview}
			<StackContentPlaceholderSelectToPreview />
		{/if}
		{#if mode === previewModes.ViewingTips && activeTip === 0}
			<StackContentTip0 />
		{/if}
		{#if mode === previewModes.ViewingTips && activeTip === 1}
			<StackContentTip1 />
		{/if}
		{#if mode === previewModes.ViewingTips && activeTip === 2}
			<StackContentTip2 />
		{/if}
	</div>
	<div class="placeholder__bottom">
		<div class="placeholder__bottom--left">
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
		<div class="placeholder__bottom--right">
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

<style>
	.placeholder {
		position: relative;
		height: 100%;
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		justify-content: flex-start;
		overflow: hidden;
	}

	.placeholder__top {
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

	.placeholder__bottom {
		width: 100%;
		display: flex;
		padding: 48px;

		border-top: 1px solid var(--clr-border-2);
	}

	.placeholder__bottom--left > div:first-child {
		margin-bottom: 20px;
	}

	.placeholder__bottom--right,
	.placeholder__bottom--left {
		flex: 0.5;
		display: flex;
		align-items: start;
		flex-direction: column;
		gap: 12px;

		& button {
			outline: none;
		}
	}

	.placeholder__bottom--right {
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
