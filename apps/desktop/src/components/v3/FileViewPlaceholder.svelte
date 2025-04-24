<script lang="ts">
	import DependentBranchTipSVG from '$lib/assets/illustrations/dependent-branch-tip.svg?raw';
	import DnDCOmmitAmendSVG from '$lib/assets/illustrations/dnd-commit-amend.svg?raw';
	import DnDCommitMoveSVG from '$lib/assets/illustrations/dnd-commit-move.svg?raw';
	import DnDCOmmitReorderSVG from '$lib/assets/illustrations/dnd-commit-reorder.svg?raw';
	import DnDCommitSquashSVG from '$lib/assets/illustrations/dnd-commit-squash.svg?raw';
	import IndependentBranchTipSVG from '$lib/assets/illustrations/independent-branch-tip.svg?raw';
	import SelectToPreviewSVG from '$lib/assets/illustrations/select-to-preview.svg?raw';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';

	type tipType = {
		title: string;
		body?: string;
		svg?: string;
		subsections?: Array<{
			title: string;
			body: string;
			svg: string;
		}>;
	};

	// VARIABLES
	const [uiState] = inject(UiState);
	const selectedTip = $derived(uiState.global.selectedTip.get().current);
	let selectedSubsectionTip = $state<number>(0);

	// TIPS DATA
	const dragndropTipSubsection = [
		{
			title: 'Amend',
			body: 'Amend commits by dragging and dropping files or specific changes, keeping your history clean and organized.',
			svg: DnDCOmmitAmendSVG
		},
		{
			title: 'Reorder',
			body: 'Reorder commits by dragging and dropping them. This works even across stacked branches, giving you full control over the commit sequence.',
			svg: DnDCOmmitReorderSVG
		},
		{
			title: 'Squash',
			body: 'Squash commits into others simply by dragging and dropping them. This helps keep your history clean by combining multiple changes into a single commit.',
			svg: DnDCommitSquashSVG
		},
		{
			title: 'Move',
			body: 'Reassign a commit to a different branch by dragging it over to its corresponding tab. This streamlines your workflow by moving commits across independent branches.',
			svg: DnDCommitMoveSVG
		}
	];

	const tipsContent: tipType[] = [
		{
			svg: DependentBranchTipSVG,
			title: 'Dependent (stacked) branches',
			body: 'GitButler lets you create a stack of branches where each branch depends on the previous one. This is useful when you have interdependent changesets that should be reviewed and merged separately (and sequentially).'
		},
		{
			svg: IndependentBranchTipSVG,
			title: 'Independent (concurrent) branches',
			body: 'GitButler lets you apply multiple independent branches (or stacks) to the workspace at the same time. This is useful when you have separate changesets that need to be reviewed and merged independently.'
		},
		{
			title: 'Drag & Drop Commit Management',
			subsections: dragndropTipSubsection
		}
	];
</script>

{#snippet tipSection(data: tipType)}
	<div
		class="tip-section"
		role="presentation"
		tabindex="-1"
		onkeydown={(e: KeyboardEvent) => {
			if (e.key === 'Escape') {
				uiState.global.selectedTip.set(undefined);
			}
		}}
	>
		<div class="tip-section__content-wrap">
			{#if data.subsections}
				<div class="tip-section__subsection">
					<div class="tip-section__illustration">
						{@html data.subsections[selectedSubsectionTip]?.svg}
					</div>

					<h3 class="text-18 text-semibold tip-section__title">
						{data.title}
					</h3>

					<div class="tip-section__subsection-menu">
						{#each data.subsections as subsection, index}
							<button
								class="text-14 text-semibold tip-section__subsection-btn"
								class:selected={selectedSubsectionTip === index}
								type="button"
								onclick={() => (selectedSubsectionTip = index)}
							>
								{subsection.title}
							</button>
						{/each}
					</div>

					{#if data.subsections[selectedSubsectionTip]}
						<p class="text-13 text-body tip-section__body">
							{data.subsections?.[selectedSubsectionTip]?.body}
						</p>
					{/if}
				</div>
			{:else}
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
			{/if}
		</div>
	</div>
{/snippet}

{#if selectedTip === undefined}
	<div class="select-some">
		<div class="select-some__image">
			{@html SelectToPreviewSVG}
		</div>
		<span class="text-13 select-some__caption">Select a file to preview</span>
	</div>
{:else}
	<Button
		class="tips-view__close-btn"
		kind="ghost"
		icon="cross"
		onclick={() => {
			uiState.global.selectedTip.set(undefined);
			selectedSubsectionTip = 0;
		}}
	/>
	{@render tipSection(tipsContent[selectedTip]!)}
{/if}

<style lang="postcss">
	/* SELECT COMMIT SECTION */
	:global(.tips-view__close-btn) {
		position: absolute;
		top: 10px;
		right: 10px;
		z-index: 1;
	}

	.select-some {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 40px 0;
		width: 100%;
		height: 100%;
	}

	.select-some__image {
		width: 100%;
		max-width: 222px;
	}

	.select-some__caption {
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

		&:focus {
			outline: none;
		}
	}

	.tip-section__content-wrap {
		max-width: 580px;
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
		text-wrap: balance;
	}

	/* SUBSECTION */
	.tip-section__subsection {
		display: flex;
		flex-direction: column;
		margin-top: 16px;
	}

	.tip-section__subsection-menu {
		display: flex;
		gap: 16px;
		margin-top: 8px;
		margin-bottom: 16px;
	}

	.tip-section__subsection-btn {
		color: var(--clr-text-3);
		text-decoration: dotted underline;
		text-underline-offset: 2px;

		&:hover {
			color: var(--clr-text-2);
		}

		&.selected {
			color: var(--clr-text-1);
			text-decoration: none;
		}
	}

	/* FOOTER */
	/* .file-view-placeholder__footer {
		display: flex;
		align-items: flex-end;
		width: 100%;
		padding: 40px;
		gap: 80px;
		border-top: 1px solid var(--clr-border-2);
	}

	.file-view-placeholder__footer__group {
		display: flex;
		flex-direction: column;
		width: fit-content;
	}

	.file-view-placeholder__footer__group-title {
		margin-bottom: 20px;
	}

	.file-view-placeholder__footer__group-list {
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
		display: flex;
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
	} */
</style>
