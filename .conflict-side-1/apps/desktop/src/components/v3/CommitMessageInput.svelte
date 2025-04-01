<script lang="ts">
	import EditorFooter from '$components/v3/editor/EditorFooter.svelte';
	import MessageEditor from '$components/v3/editor/MessageEditor.svelte';
	import { persistedCommitMessage } from '$lib/config/config';
	import Button from '@gitbutler/ui/Button.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';

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
		initialMessage: initialValue
	}: Props = $props();

	let titleText = $state<string | undefined>(initialTitle);
	let descriptionText = $state<string | undefined>(initialValue);
	const commitMessage = persistedCommitMessage(projectId, stackId);

	$effect(() => {
		if (isNewCommit) {
			$commitMessage = [titleText, descriptionText].filter((a) => a).join('\n\n');
		}
	});

	let composer = $state<ReturnType<typeof MessageEditor>>();

	export function getMessage() {
		return $commitMessage;
	}
</script>

<div class="commit-message-input">
	<Textbox bind:value={titleText} placeholder="Commit title" />
	<MessageEditor
		bind:this={composer}
		{initialValue}
		{projectId}
		{stackId}
		onChange={(text: string) => {
			descriptionText = text;
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
