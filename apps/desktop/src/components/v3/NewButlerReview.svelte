<script lang="ts">
	import Drawer from './Drawer.svelte';
	import CommitMessageEditor from './editor/CommitMessageEditor.svelte';
	import EditorFooter from './editor/EditorFooter.svelte';
	import EditorHeader from './editor/EditorHeader.svelte';
	import { stackPath } from '$lib/routes/routes.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import { writable } from 'svelte/store';
	import { goto } from '$app/navigation';

	type Props = {
		projectId: string;
		stackId: string;
	};

	const { projectId, stackId }: Props = $props();

	let markdown = writable(true);
	let composer: CommitMessageEditor | undefined = $state();
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
	<CommitMessageEditor bind:this={composer} bind:markdown={$markdown} />
	<EditorFooter onCancel={cancel}>
		<Button style="pop" onclick={createPr} wide>Create Butler review</Button>
	</EditorFooter>
</Drawer>
