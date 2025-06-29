<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import Feed from '$components/v3/Feed.svelte';
	import MainViewport from '$components/v3/MainViewport.svelte';
	import MultiStackView from '$components/v3/MultiStackView.svelte';
	import SelectionView from '$components/v3/SelectionView.svelte';
	import UnassignedView from '$components/v3/UnassignedView.svelte';
	import { SettingsService } from '$lib/config/appSettingsV2';
	import {
		DefinedFocusable,
		FocusManager,
		parseFocusableId,
		stackFocusableId,
		uncommittedFocusableId
	} from '$lib/focus/focusManager.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { setContext } from 'svelte';
	import { writable } from 'svelte/store';
	import type { SelectionId } from '$lib/selection/key';

	interface Props {
		projectId: string;
		stackId?: string;
	}

	const { stackId, projectId }: Props = $props();

	const [stackService, focusManager, idSelection, uiState, settingsService] = inject(
		StackService,
		FocusManager,
		IdSelection,
		UiState,
		SettingsService
	);
	const worktreeSelection = $derived(idSelection.getById({ type: 'worktree' }));
	const stacksResult = $derived(stackService.stacks(projectId));
	const projectState = $derived(uiState.project(projectId));
	const settingsStore = $derived(settingsService.appSettings);
	const canUseActions = $derived($settingsStore?.featureFlags.actions ?? false);
	const showingActions = $derived(projectState.showActions.current && canUseActions);

	const snapshotFocusables = writable<string[]>([]);
	setContext('snapshot-focusables', snapshotFocusables);

	const stackFocusables = $derived(
		stacksResult.current?.data
			? stacksResult.current.data.map((stack) => stackFocusableId(stack.id))
			: []
	);

	const uncommittedFocusables = $derived(
		stacksResult.current?.data
			? stacksResult.current.data.map((stack) => uncommittedFocusableId(stack.id))
			: []
	);

	let focusGroup = $derived(
		focusManager.radioGroup({
			triggers: [
				DefinedFocusable.UncommittedChanges,
				DefinedFocusable.Drawer,
				...stackFocusables,
				...$snapshotFocusables,
				...uncommittedFocusables
			]
		})
	);

	const focusedStackId = $derived(
		focusGroup.current ? parseFocusableId(focusGroup.current) : undefined
	);

	const selectionId = { type: 'worktree', stackId: undefined } as SelectionId;

	const lastAdded = $derived(worktreeSelection.lastAdded);
	const previewOpen = $derived(!!$lastAdded?.key);
</script>

{#snippet drawerRight()}
	<Feed {projectId} onCloseClick={() => uiState.project(projectId).showActions.set(false)} />
{/snippet}

{#snippet leftSideview()}
	<SelectionView {projectId} {selectionId} draggableFiles />
{/snippet}

<MainViewport
	name="workspace"
	leftSectionWidth={{ default: 280, min: 240 }}
	leftSideviewWidth={{ default: 380, min: 240 }}
	rightSideview={showingActions ? drawerRight : undefined}
	leftSideview={previewOpen ? leftSideview : undefined}
>
	{#snippet leftSection()}
		<UnassignedView {projectId} focus={focusGroup.current as DefinedFocusable} />
	{/snippet}
	{#snippet mainSection()}
		<ReduxResult {projectId} result={stacksResult?.current}>
			{#snippet loading()}
				<div class="stacks-view-skeleton"></div>
			{/snippet}
			{#snippet children(stacks, { projectId })}
				<MultiStackView {projectId} {stacks} {selectionId} selectedId={stackId} {focusedStackId} />
			{/snippet}
		</ReduxResult>
	{/snippet}
</MainViewport>

<style>
	.stacks-view-skeleton {
		width: 100%;
		height: 100%;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}
</style>
