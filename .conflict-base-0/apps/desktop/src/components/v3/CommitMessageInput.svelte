<script lang="ts">
	import EditorFooter from '$components/v3/editor/EditorFooter.svelte';
	import MessageEditor from '$components/v3/editor/MessageEditor.svelte';
	import { persisted } from '@gitbutler/shared/persisted';
	import Button from '@gitbutler/ui/Button.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';

	type Props = {
		projectId: string;
		action: () => void;
		actionLabel: string;
		onCancel: () => void;
		disabledAction?: boolean;
		loading?: boolean;
		initialTitle?: string;
		initialMessage?: string;
	};

	const {
		projectId,
		actionLabel,
		action,
		onCancel,
		disabledAction,
		loading,
		initialTitle,
		initialMessage: initialValue
	}: Props = $props();

	/**
	 * Toggles use of markdown on/off in the message editor.
	 */
	let markdown = persisted(true, 'useMarkdown__' + projectId);

	let titleText = $state<string | undefined>(initialTitle);
	let composer = $state<ReturnType<typeof MessageEditor>>();

	export function getTitle(): string | undefined {
		return titleText;
	}

	export async function getPlaintext(): Promise<string | undefined> {
		return await composer?.getPlaintext();
	}
</script>

<div class="commit-message-input">
	<Textbox bind:value={titleText} placeholder="Commit title" />
	<MessageEditor bind:this={composer} bind:markdown={$markdown} {initialValue} />
</div>
<EditorFooter {onCancel}>
	<Button style="pop" onclick={action} disabled={disabledAction} {loading}>{actionLabel}</Button>
</EditorFooter>

<style lang="postcss">
	.commit-message-input {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
</style>
