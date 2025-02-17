<script lang="ts" module>
	// eslint-disable-next-line svelte/valid-compile
	export enum PreviewMode {
		EmptyBranch,
		SelectToPreview,
		Tip1,
		Tip2,
		Tip3
	}

	// eslint-disable-next-line svelte/valid-compile
	enum PlaceholderType {
		Tip,
		Placeholder
	}

	const modeMap = {
		[PreviewMode.EmptyBranch]: {
			svg: EmptyStack,
			type: PlaceholderType.Placeholder,
			title: 'This is a new stack',
			body: `
				Stack is a workflow for building branches sequentially to
				break features into smaller parts. You can also choose a
				regular single-branch flow.
			`
		},
		[PreviewMode.SelectToPreview]: {
			svg: SelectACommit,
			type: PlaceholderType.Placeholder,
			title: 'Select a commit to preview',
			body: ''
		},
		[PreviewMode.Tip1]: {
			svg: WhatIsAStack,
			type: PlaceholderType.Tip,
			title: 'What is a stack',
			body: `
				Stack is a workflow where branches are built sequentially,
				breaking large features into smaller parts. Each branch depends
				on the previous one. You can also choose a single-branch flow.
			`
		},
		[PreviewMode.Tip2]: {
			svg: CommitAndPush,
			type: PlaceholderType.Tip,
			title: 'Commit and push',
			body: `
				File changes can be committed in any stack unless already
				committed in another, as they are dependent on a branch.
				When the dependent branch is selected, you can commit
				“locked” files. All branches in a stack are pushed together.
			`
		},
		[PreviewMode.Tip3]: {
			svg: ManageCommits,
			type: PlaceholderType.Tip,
			title: 'Manage commits',
			body: `
				File changes can be committed in any stack unless already
				committed in another, as they are dependent on a branch.
				When the dependent branch is selected, you can commit
				“locked” files. All branches in a stack are pushed together.
			`
		}
	};
</script>

<script lang="ts">
	import StackContentTip from '$components/v3/StackContentTip.svelte';
	import EmptyStack from '$lib/assets/illustrations/empty-stack-placeholder.svg?raw';
	import SelectACommit from '$lib/assets/illustrations/select-a-commit-preview.svg?raw';
	import CommitAndPush from '$lib/assets/illustrations/tip-commit-and-push.svg?raw';
	import ManageCommits from '$lib/assets/illustrations/tip-manage-commits.svg?raw';
	import WhatIsAStack from '$lib/assets/illustrations/tip-what-is-a-stack.svg?raw';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
	import { slide } from 'svelte/transition';

	interface Props {
		mode: PreviewMode;
	}

	let { mode }: Props = $props();
	const { svg, title, body, type } = $derived(modeMap[mode]);
</script>

{#snippet tipButton(value: PreviewMode)}
	{@const selected = value === mode}
	<button
		type="button"
		class="tip text-13 text-semibold"
		class:selected
		onclick={() => (mode = value)}
	>
		{#if value === mode}
			<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
		{/if}
		{modeMap[value].title}
	</button>
{/snippet}

<div class="placeholder">
	<div class="top" class:white-bg={type === PlaceholderType.Tip}>
		<StackContentTip {title} {body}>
			{#snippet illustration()}
				{@html svg}
			{/snippet}
		</StackContentTip>
	</div>
	<div class="bottom">
		<div class="content">
			<div class="left">
				<div class="text-16 text-semibold">Tips</div>
				{@render tipButton(PreviewMode.Tip1)}
				{@render tipButton(PreviewMode.Tip2)}
				{@render tipButton(PreviewMode.Tip3)}
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

	.tip {
		color: var(--clr-text-3);
	}
	.selected {
		color: var(--clr-text-2);
	}
</style>
