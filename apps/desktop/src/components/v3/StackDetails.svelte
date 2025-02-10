<script lang="ts">
	import { isArchivedBranch, isStackedBranch } from './lib';
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import Branch from '$components/v3/Branch.svelte';
	import StackContentIllustration, {
		PreviewMode
	} from '$components/v3/StackContentIllustration.svelte';
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
		if (!stackData) return PreviewMode.EmptyBranch;
		if (isArchivedBranch(stackData.state)) return PreviewMode.SelectToPreview;

		if (isStackedBranch(stackData.state) && stackData.state.subject.localAndRemote.length === 0) {
			return PreviewMode.EmptyBranch;
		}

		return PreviewMode.SelectToPreview;
	});
</script>

<div class="stack">
	<div
		class="stack__wrapper"
		bind:this={resizeStackBranches}
		style:width={$stackBranchWidth + 'rem'}
	>
		<Resizer
			viewport={resizeStackBranches}
			direction="right"
			minWidth={22.5}
			onWidth={(value) => {
				$stackBranchWidth = value / (16 * $userSettings.zoom);
			}}
		/>
		<div
			class="stack__branches"
			bind:this={resizeStackBranches}
			style:width={$stackBranchWidth + 'rem'}
		>
			<ReduxResult result={result.current}>
				{#snippet children(result)}
					{#if stackId && result.length > 0}
						{#each result as branch, i (branch.name)}
							{@const first = i === 0}
							{@const last = i === result.length - 1}
							<Branch {branch} {first} {last} />
						{/each}
					{/if}
				{/snippet}
			</ReduxResult>
		</div>
	</div>

	<div class="stack__branch-content">
		<StackContentIllustration mode={stackContentMode} />
	</div>
</div>

<style>
	.stack__wrapper,
	.stack {
		position: relative;
		height: 100%;
		display: flex;
		border-radius: 0 var(--radius-ml) var(--radius-ml);
	}

	.stack__branches {
		position: relative;
		display: flex;
		width: 22.5rem;
		flex-direction: column;
		padding: 16px;
		overflow: hidden;

		background-color: transparent;
		opacity: 1;
		background-image: radial-gradient(var(--clr-border-2) 0.9px, #ffffff00 0.9px);
		background-size: 12px 12px;
		border-right: 1px solid var(--clr-border-2);
	}

	.stack__branch-content {
		display: flex;
		flex: 1;
		flex-direction: column;
	}
</style>
