<script lang="ts">
	import EditorFooter from '$components/v3/editor/EditorFooter.svelte';
	import MessageEditor from '$components/v3/editor/MessageEditor.svelte';
	import { persistedCommitMessage } from '$lib/config/config';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';

	type Props = {
		isNewCommit?: boolean;
		projectId: string;
		stackId: string;
		actionLabel: string;
		action: () => void;
		onCancel: () => void;
		disabledAction?: boolean;
		loading?: boolean;
		initialTitle?: string;
		initialMessage?: string;
	};

	const {
		isNewCommit,
		projectId,
		stackId,
		actionLabel,
		action,
		onCancel,
		disabledAction,
		loading,
		initialTitle,
		initialMessage
	}: Props = $props();

	const uiState = getContext(UiState);

	const titleText = $derived(uiState.project(projectId).commitTitle);
	const descriptionText = $derived(uiState.project(projectId).commitMessage);

	$effect(() => {
		if (isDefined(initialTitle)) {
			titleText.current = initialTitle;
		}

		if (isDefined(initialMessage)) {
			descriptionText.current = initialMessage;
		}
	});

	const commitMessage = persistedCommitMessage(projectId, stackId);

	$effect(() => {
		if (isNewCommit) {
			$commitMessage = [titleText.current, descriptionText.current].filter((a) => a).join('\n\n');
		}
	});

	let composer = $state<ReturnType<typeof MessageEditor>>();

	export function getMessage() {
		return $commitMessage;
	}
</script>

<div class="commit-message-input">
	<Textbox
		autofocus
		size="large"
		placeholder="Commit title"
		value={titleText.current}
		oninput={(value: string) => {
			titleText.set(value);
		}}
	/>
	<MessageEditor
		bind:this={composer}
		initialValue={descriptionText.current}
		placeholder={'Your commit message'}
		{projectId}
		{stackId}
		onChange={(text: string) => {
			descriptionText.current = text;
		}}
	/>
</div>
<EditorFooter {onCancel}>
	<Button style="pop" onclick={action} disabled={disabledAction} {loading} width={126}
		>{actionLabel}</Button
	>
</EditorFooter>

<style lang="postcss">
	.commit-message-input {
		flex: 1;
		display: flex;
		flex-direction: column;
		min-height: 0;
		gap: 10px;
	}
</style>
