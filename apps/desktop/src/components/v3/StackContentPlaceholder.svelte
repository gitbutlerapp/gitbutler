<script lang="ts">
	import EmptyStack from '$lib/assets/illustrations/empty-stack-placeholder.svg?raw';
	import SelectACommitSVG from '$lib/assets/illustrations/select-a-commit-preview.svg?raw';
	import CommitAndPushSVG from '$lib/assets/illustrations/tip-commit-and-push.svg?raw';
	import ManageCommitsSVG from '$lib/assets/illustrations/tip-manage-commits.svg?raw';
	import WhatIsAStackSVG from '$lib/assets/illustrations/tip-what-is-a-stack.svg?raw';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { slide } from 'svelte/transition';

	interface Props {
		isNewStack?: boolean;
	}

	let { isNewStack }: Props = $props();

	type tipType = {
		placeholder: boolean;
		svg: string;
		title: string;
		body: string;
	};

	const tipsContent = {
		newStack: {
			placeholder: false,
			svg: EmptyStack,
			title: 'This is a new stack',
			body: `
				Stack is a workflow for building branches sequentially to break
				features into smaller parts. You can also choose a regular 
				single-branch flow.
			`
		},
		tip1: {
			placeholder: true,
			svg: WhatIsAStackSVG,
			title: 'What is a stack',
			body: `
				Stack is a workflow where branches are built sequentially,
				breaking large features into smaller parts. Each branch depends
				on the previous one. You can also choose a single-branch flow.
			`
		},
		tip2: {
			placeholder: true,
			svg: CommitAndPushSVG,
			title: 'Commit and push',
			body: `
				File changes can be committed in any stack unless already
				committed in another, as they are dependent on a branch.
				When the dependent branch is selected, you can commit
				“locked” files. All branches in a stack are pushed together.
			`
		},
		tip3: {
			placeholder: true,
			svg: ManageCommitsSVG,
			title: 'Manage commits',
			body: `
				File changes can be committed in any stack unless already
				committed in another, as they are dependent on a branch.
				When the dependent branch is selected, you can commit
				“locked” files. All branches in a stack are pushed together.
			`
		}
	};

	let selectedTipKey = $state<keyof typeof tipsContent | undefined>();
</script>

{#snippet tipButton(key: keyof typeof tipsContent)}
	{@const data = tipsContent[key]}
	{@const selected = selectedTipKey === key}
	<button
		type="button"
		class="text-13 text-semibold text-body tip-button"
		class:selected
		onclick={() => (selectedTipKey = key)}
	>
		{#if selected}
			<div class="active-page-indicator" in:slide={{ axis: 'x', duration: 150 }}></div>
		{/if}
		{data.title}
	</button>
{/snippet}

{#snippet tipSection(data: tipType)}
	<div class="tip-section" class:is-placeholder={!data.placeholder}>
		<div class="tip-section__content-wrap">
			<div class="tip-section__illustration">
				{@html data.svg}
			</div>

			<h3 class="text-18 text-semibold tip-section__title">
				{data.title}
			</h3>
			<p class="text-13 text-body tip-section__body">
				{data.body}
			</p>
		</div>
	</div>
{/snippet}

<div class="stack-placeholder">
	<div class="stack-placeholder__top">
		{#if !selectedTipKey}
			{#if !isNewStack}
				<div class="select-commit-state">
					<div class="select-commit-state__image">
						{@html SelectACommitSVG}
					</div>
					<span class="text-13 select-commit-state__caption">Select a commit to preview</span>
				</div>
			{:else}
				{@render tipSection(tipsContent.newStack)}
			{/if}
		{:else}
			<Button
				class="tip-close-btn"
				type="button"
				kind="ghost"
				icon="cross"
				onclick={() => (selectedTipKey = undefined)}
			></Button>
			{@render tipSection(tipsContent[selectedTipKey])}
		{/if}
	</div>
	<div class="stack-placeholder__footer">
		<div class="stack-placeholder__footer__group">
			<h3 class="text-16 text-semibold stack-placeholder__footer__group-title">Tips</h3>
			<div class="stack-placeholder__footer__group-list">
				{@render tipButton('tip1')}
				{@render tipButton('tip2')}
				{@render tipButton('tip3')}
			</div>
		</div>
		<div class="stack-placeholder__footer__group">
			<div class="stack-placeholder__footer__group-list">
				<a
					type="button"
					href="https://docs.gitbutler.com"
					target="_blank"
					class="text-13 text-semibold text-body external-link"
					><Icon name="doc" /> <span class="external-link__label">GitButler Docs</span>
					<div class="external-link__link-icon"><Icon name="open-link" /></div></a
				>
				<a
					type="button"
					href="https://github.com/gitbutlerapp/gitbutler"
					target="_blank"
					class="text-13 text-semibold text-body external-link"
					><Icon name="github" /> <span class="external-link__label">Source Code</span>
					<div class="external-link__link-icon"><Icon name="open-link" /></div></a
				>
				<a
					type="button"
					href="https://discord.com/invite/MmFkmaJ42D"
					target="_blank"
					class="text-13 text-semibold text-body external-link"
					><Icon name="discord" /> <span class="external-link__label">Join Community</span>
					<div class="external-link__link-icon"><Icon name="open-link" /></div></a
				>
			</div>
		</div>
	</div>
</div>

<style>
	.stack-placeholder {
		flex: 1;
		position: relative;
		height: 100%;
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		justify-content: flex-start;
		overflow: hidden;
	}

	.stack-placeholder__top {
		flex: 1;
		width: 100%;
		display: flex;
		flex-direction: column;
		justify-content: center;
		gap: 24px;

		& :global(.tip-close-btn) {
			position: absolute;
			top: 8px;
			right: 8px;
		}
	}

	/* SELECT COMMIT SECTION */
	.select-commit-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
	}

	.select-commit-state__image {
		width: 100%;
		max-width: 222px;
	}

	.select-commit-state__caption {
		margin-top: 28px;
		color: var(--clr-text-2);
		opacity: 0.6;
	}

	/* TIP SECTION */
	.tip-section {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
		height: 100%;

		&:not(.is-placeholder) {
			background-color: var(--clr-bg-1);
		}
	}

	.tip-section__content-wrap {
		max-width: 600px;
		padding: 48px;
	}

	.tip-section__illustration {
		width: 100%;
		max-width: 300px;
		margin-bottom: 28px;
	}

	.tip-section__title {
		margin-bottom: 10px;
	}

	.tip-section__body {
		color: var(--clr-text-2);
	}

	/* FOOTER */
	.stack-placeholder__footer {
		display: flex;
		align-items: flex-end;
		width: 100%;
		padding: 40px;
		gap: 80px;
		border-top: 1px solid var(--clr-border-2);
	}

	.stack-placeholder__footer__group {
		display: flex;
		flex-direction: column;
		width: fit-content;
	}

	.stack-placeholder__footer__group-title {
		margin-bottom: 20px;
	}

	.stack-placeholder__footer__group-list {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.tip-button {
		text-align: left;
		color: var(--clr-text-2);
		transition:
			color var(--transition-fast),
			opacity var(--transition-fast);
		opacity: 0.7;

		&:hover {
			color: var(--clr-text-1);
		}

		&:last-child {
			margin-bottom: 0;
		}

		&.selected {
			color: var(--clr-text-1);
			opacity: 1;
		}
	}

	.external-link {
		display: flex;
		align-items: center;
		color: var(--clr-text-2);
		gap: 12px;

		&:hover {
			& .external-link__label {
				color: var(--clr-text-1);
			}

			& .external-link__link-icon {
				opacity: 1;
				transform: translateX(-40%) scale(1);
			}
		}
	}

	.external-link__link-icon {
		opacity: 0;
		transform: translateX(-45%) scale(0.8);
		transition:
			opacity var(--transition-fast),
			transform var(--transition-medium);
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
