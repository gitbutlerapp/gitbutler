<script lang="ts">
	import DependentBranchTipSVG from '$lib/assets/illustrations/dependent-branch-tip.svg?raw';
	import IndependentBranchTipSVG from '$lib/assets/illustrations/independent-branch-tip.svg?raw';
	import NewBranchSVG from '$lib/assets/illustrations/new-branch.svg?raw';
	import SelectACommitSVG from '$lib/assets/illustrations/select-a-commit-preview.svg?raw';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { slide } from 'svelte/transition';

	interface Props {
		isNewStack?: boolean;
	}

	let { isNewStack }: Props = $props();

	type tipType = {
		title: string;
		body?: string;
		svg?: string;
		linkLabel?: string;
		subsections?: Record<string, { title: string; body: string }>;
	};

	const newStackTip: tipType = {
		svg: NewBranchSVG,
		title: 'This is a new branch',
		body: 'You can commit your changes here. Additional branches can be stacked on top of this one or applied independently to the workspace.'
	};

	const dragndropTipSubsection = {
		amend: {
			title: 'Amend',
			body: 'Amend commits by dragging and dropping files or specific changes, keeping your history clean and organized.'
		},
		reorder: {
			title: 'Reorder',
			body: 'Reorder commits by dragging and dropping them. This works even across stacked branches, giving you full control over the commit sequence.'
		},
		squash: {
			title: 'Squash',
			body: 'Squash commits into others simply by dragging and dropping them. This helps keep your history clean by combining multiple changes into a single commit.'
		},
		move: {
			title: 'Move',
			body: 'Reassign a commit to a different branch by dragging it over to its corresponding tab. This streamlines your workflow by moving commits across independent branches.'
		}
	};

	// WORK IN PROGRESS: Add illustrations for the subsections
	// when we decide how V3 layout will look like

	const tipsContent = {
		tip1: {
			svg: DependentBranchTipSVG,
			linkLabel: 'Dependent branches',
			title: 'Dependent (stacked) branches',
			body: 'GitButler lets you create a stack of branches where each branch depends on the previous one. This is useful when you have interdependent changesets that should be reviewed and merged separately (and sequentially).'
		} as tipType,
		tip2: {
			svg: IndependentBranchTipSVG,
			linkLabel: 'Independent branches',
			title: 'Independent (concurrent) branches',
			body: 'GitButler lets you apply multiple independent branches (or stacks) to the workspace at the same time. This is useful when you have separate changesets that need to be reviewed and merged independently.'
		} as tipType,
		tip3: {
			linkLabel: 'Drag & Drop Commits',
			title: 'Drag & Drop Commit Management',
			subsections: dragndropTipSubsection
		} as tipType
	};

	let selectedTipKey = $state<keyof typeof tipsContent | undefined>('tip3');
	let selectedDragndropTip = $state<keyof typeof dragndropTipSubsection>('amend');
</script>

{#snippet subSectionMenu({
	key,
	label
}: {
	key: keyof typeof dragndropTipSubsection;
	label: string;
})}
	{@const selected = selectedDragndropTip === key}
	<button
		type="button"
		class="text-13 text-semibold text-body tip-button"
		class:selected
		onclick={() => (selectedDragndropTip = key)}
	>
		{label}
	</button>
{/snippet}

{#snippet tipButton(props: { key: keyof typeof tipsContent; label: string })}
	{@const { key, label } = props}
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
		{label}
	</button>
{/snippet}

{#snippet tipSection(props: { data: tipType; placeholder?: boolean })}
	{@const { data, placeholder } = props}
	<div class="tip-section" class:is-placeholder={placeholder}>
		<div class="tip-section__content-wrap">
			<div class="tip-section__illustration">
				{@html data.svg}
			</div>

			<h3 class="text-18 text-semibold tip-section__title">
				{data.title}
			</h3>

			{#if data.body}
				<p class="text-13 text-body tip-section__body">
					{data.body}
				</p>
			{/if}

			{#if data.subsections}
				<div class="tip-section__subsection">
					{#each Object.entries(data.subsections) as [key, subsection]}
						{@render subSectionMenu({
							key: key as keyof typeof dragndropTipSubsection,
							label: subsection.title
						})}
					{/each}

					{#if data.subsections[selectedDragndropTip]}
						<p class="text-16 text-semibold">
							{data.subsections?.[selectedDragndropTip]?.body}
						</p>
					{/if}
				</div>
			{/if}
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
				{@render tipSection({
					data: newStackTip,
					placeholder: true
				})}
			{/if}
		{:else}
			<Button
				class="tip-close-btn"
				type="button"
				kind="ghost"
				icon="cross"
				onclick={() => (selectedTipKey = undefined)}
			></Button>
			{@render tipSection({
				data: tipsContent[selectedTipKey]!,
				placeholder: false
			})}
		{/if}
	</div>
	<div class="stack-placeholder__footer">
		<div class="stack-placeholder__footer__group">
			<h3 class="text-16 text-semibold stack-placeholder__footer__group-title">Tips</h3>
			<div class="stack-placeholder__footer__group-list">
				{@render tipButton({
					key: 'tip1',
					label: 'Dependent branches'
				})}
				{@render tipButton({
					key: 'tip2',
					label: 'Independent branches'
				})}
				{@render tipButton({
					key: 'tip3',
					label: 'Drag & Drop Commits'
				})}
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
