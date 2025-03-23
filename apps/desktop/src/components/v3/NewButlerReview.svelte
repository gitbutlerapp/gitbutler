<script lang="ts">
	import Drawer from '$components/v3/Drawer.svelte';
	import EditorFooter from '$components/v3/editor/EditorFooter.svelte';
	import EditorHeader from '$components/v3/editor/EditorHeader.svelte';
	import MessageEditor from '$components/v3/editor/MessageEditor.svelte';
	import TitleInput from '$components/v3/editor/TitleInput.svelte';
	import { stackPath } from '$lib/routes/routes.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import { writable } from 'svelte/store';
	import { goto } from '$app/navigation';

	type Props = {
		projectId: string;
		stackId: string;
	};

	const { projectId, stackId }: Props = $props();

	let titleText = $state<string>();
	let markdown = writable(true);
	let composer: MessageEditor | undefined = $state();
	let drawer = $state<ReturnType<typeof Drawer>>();

	function createPr() {
		throw new Error('Not implemented!');
	}

	function cancel() {
		drawer?.onClose();
		goto(stackPath(projectId, stackId));
	}
</script>

<Drawer bind:this={drawer} {projectId} {stackId}>
	<EditorHeader title="New Butler review" bind:markdown={$markdown} />
	<TitleInput bind:value={titleText} />
	<MessageEditor bind:this={composer} bind:markdown={$markdown} {projectId} {stackId} />
	<EditorFooter onCancel={cancel}>
		<Button style="pop" onclick={createPr} wide>Create Butler review</Button>
	</EditorFooter>
</Drawer>
