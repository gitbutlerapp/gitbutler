<script lang="ts">
	import ReviewGoesHere from './ReviewGoesHere.svelte';
	import CommitMessageEditor from './editor/CommitMessageEditor.svelte';
	import EditorFooter from './editor/EditorFooter.svelte';
	import EditorHeader from './editor/EditorHeader.svelte';
	import ResizeableSplitLayout from '$components/v3/ResizeableSplitLayout.svelte';
	import { stackPath } from '$lib/routes/routes.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import { writable } from 'svelte/store';
	import { goto } from '$app/navigation';

	type Props = {
		projectId: string;
		stackId: string;
		branchName: string;
	};

	const { projectId, stackId, branchName }: Props = $props();

	let markdown = writable(true);
	let composer: CommitMessageEditor | undefined = $state();

	function createPr() {
		throw new Error('Not implemented!');
	}
</script>

<ResizeableSplitLayout {projectId}>
	{#snippet left()}
		<ReviewGoesHere {projectId} {stackId} {branchName} />
	{/snippet}
	{#snippet main()}
		<EditorHeader title="New Butler review" bind:markdown={$markdown} />
		<CommitMessageEditor bind:this={composer} bind:markdown={$markdown} />
		<EditorFooter onCancel={() => goto(stackPath(projectId, stackId))}>
			<Button style="pop" onclick={createPr} wide>Create pull request</Button>
		</EditorFooter>
	{/snippet}
</ResizeableSplitLayout>

<style lang="postcss">
</style>
