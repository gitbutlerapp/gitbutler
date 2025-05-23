<script lang="ts">
	import SelectTopreviewPlaceholder from '$components/v3/SelectTopreviewPlaceholder.svelte';
	import DependentBranchTipSVG from '$lib/assets/illustrations/dependent-branch-tip.svg?raw';
	import DnDCOmmitAmendSVG from '$lib/assets/illustrations/dnd-commit-amend.svg?raw';
	import DnDCommitMoveSVG from '$lib/assets/illustrations/dnd-commit-move.svg?raw';
	import DnDCOmmitReorderSVG from '$lib/assets/illustrations/dnd-commit-reorder.svg?raw';
	import DnDCommitSquashSVG from '$lib/assets/illustrations/dnd-commit-squash.svg?raw';
	import IndependentBranchTipSVG from '$lib/assets/illustrations/independent-branch-tip.svg?raw';
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
			body: 'Reassign a commit to a different branch by dragging it over to its corresponding lane. This streamlines your workflow by moving commits across independent branches.',
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
	<SelectTopreviewPlaceholder />
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
		z-index: 1;
		position: absolute;
		top: 10px;
		right: 10px;
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
		margin-top: 8px;
		margin-bottom: 16px;
		gap: 16px;
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
</style>
