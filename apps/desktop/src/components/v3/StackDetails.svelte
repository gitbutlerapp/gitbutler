<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import Branch from '$components/v3/Branch.svelte';
	import StackCommitDetails from '$components/v3/StackCommitDetails.svelte';
	import StackContentIllustration, {
		PreviewMode
	} from '$components/v3/StackContentIllustration.svelte';
	import { isStackedBranch } from '$components/v3/lib';
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext, getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';

	interface Props {
		stackId: string;
		projectId: string;
	}

	const { stackId, projectId }: Props = $props();

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
	let resizeStackBranches = $state<HTMLElement>();
	const stackBranchWidthKey = $derived('defaultStackBranchWidth_ ' + projectId);
	let stackBranchWidth = $derived(persisted<number>(22.5, stackBranchWidthKey));

	const stackService = getContext(StackService);
	const result = $derived(stackService.getStackBranches(projectId, stackId));
	const stackData = $derived(result.current.data?.[0]);

	const stackContentMode = $derived.by<PreviewMode>(() => {
		if (
			!stackData ||
			(isStackedBranch(stackData.state) && stackData.state.subject.localAndRemote.length === 0)
		) {
			return PreviewMode.EmptyBranch;
		}

		return PreviewMode.SelectToPreview;
	});

	let selectedCommitId = $state<string>();
	$inspect('stackDetails.selectedCommitId', selectedCommitId);
</script>

<div class="wrapper">
	<div class="branches" bind:this={resizeStackBranches} style:width={$stackBranchWidth + 'rem'}>
		<Resizer
			viewport={resizeStackBranches}
			direction="right"
			minWidth={22.5}
			onWidth={(value) => {
				$stackBranchWidth = value / (16 * $userSettings.zoom);
			}}
		/>
		<ReduxResult result={result.current}>
			{#snippet children(result)}
				{#if stackId && result.length > 0}
					{#each result as branch, i (branch.name)}
						{@const first = i === 0}
						{@const last = i === result.length - 1}
						<Branch {branch} {first} {last} bind:selectedCommitId />
					{/each}
				{/if}
			{/snippet}
		</ReduxResult>
	</div>

	{#if selectedCommitId}
		<StackCommitDetails bind:selectedCommitId />
	{:else}
		<StackContentIllustration mode={stackContentMode} />
	{/if}
</div>

<style>
	.wrapper {
		position: relative;
		height: 100%;
		display: flex;
		border-radius: 0 var(--radius-ml) var(--radius-ml);
	}

	.branches {
		position: relative;
		display: flex;
		width: 22.5rem;
		flex-direction: column;
		padding: 16px;
		overflow: hidden;

		background-color: transparent;
		opacity: 1;
		background-image: radial-gradient(
			oklch(from var(--clr-scale-ntrl-50) l c h / 0.5) 0.6px,
			#ffffff00 0.6px
		);
		background-size: 6px 6px;
		border-right: 1px solid var(--clr-border-2);
	}
</style>
