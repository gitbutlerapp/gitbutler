<script lang="ts">
	import StackingStatusIcon from './StackingStatusIcon.svelte';
	import { Project } from '$lib/backend/projects';
	import { projectShowStackingCardDetails } from '$lib/config/config';
	import Link from '$lib/shared/Link.svelte';
	import Spacer from '$lib/shared/Spacer.svelte';
	import { getContext } from '$lib/utils/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';

	interface Props {
		addSeries: (e: MouseEvent) => void;
		branchId: string;
	}

	const { branchId: _branchId, addSeries }: Props = $props();

	const project = getContext(Project);

	const showStackingCardDetails = projectShowStackingCardDetails(project.id);
	let showDetails = $state($showStackingCardDetails);

	function closeStackingCard() {
		showStackingCardDetails.set(false);
		showDetails = false;
	}

	async function addSeriesToStack(e: MouseEvent) {
		addSeries(e);
	}
</script>

<section class="card">
	{#if showDetails}
		<button tabindex="0" class="card__close" onclick={closeStackingCard}>
			<Icon name="cross-small" />
		</button>
		<div class="card__body">
			<h2 class="text-16 text-bold">New branch stacking</h2>
			<p class="text-12 card__description">
				Allows you to add a branch that depends on previous branches. This helps you create smaller
				PRs that are reviewed and merged in order.
				<Link href="https://docs.gitbutler.com/stacking" target="_blank">Read more</Link>
			</p>
		</div>
		<Spacer />
	{/if}
	<section class="card__action" class:showDetails={!showDetails}>
		<StackingStatusIcon icon="plus-small" color="neutral" gap={true} />
		<Button grow style="neutral" onclick={addSeriesToStack}>Add a branch to the stack</Button>
	</section>
</section>

<style>
	.card {
		position: relative;
		display: flex;
		flex-direction: column;
	}

	.card__body {
		padding: 16px 16px 0 16px;
	}

	.card__close {
		position: absolute;
		top: 8px;
		right: 8px;

		color: var(--clr-scale-ntrl-60);
	}

	.card__description {
		color: var(--clr-scale-ntrl-50);
		line-height: 18px;
	}

	.card__action {
		width: 100%;
		display: flex;
		justify-content: around;
		align-items: flex-start;
		padding: 0 13px;
		gap: 1rem;

		&.showDetails {
			margin-top: 16px;
		}
	}
</style>
