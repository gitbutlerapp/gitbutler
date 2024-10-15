<script lang="ts">
	import Markdown from '$lib/components/Markdown.svelte';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import CreateIssueModal from '$lib/topics/CreateIssueModal.svelte';
	import CreateTopicModal from '$lib/topics/CreateTopicModal.svelte';
	import { TopicService, type Topic } from '$lib/topics/service';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';

	interface Props {
		topic: Topic;
	}

	const { topic }: Props = $props();

	const topicService = getContext(TopicService);
	const gitHost = getGitHost();

	let deleteModal = $state<Modal>();

	let expanded = $state(false);

	let createIssueModal = $state<CreateIssueModal>();
	let createTopicModal = $state<CreateIssueModal>();
</script>

<CreateIssueModal bind:this={createIssueModal} {topic} />
<CreateTopicModal bind:this={createTopicModal} {topic} />

<Modal
	bind:this={deleteModal}
	width="small"
	onSubmit={() => {
		topicService.remove(topic);
		deleteModal?.close();
	}}
>
	<p>Are you sure you want to delete this topic?</p>
	{#snippet controls()}
		<Button onclick={() => deleteModal?.close()}>Cancel</Button>
		<Button type="submit" kind="solid" style="error">Delete</Button>
	{/snippet}
</Modal>

<div class="topic">
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="header" onclick={() => (expanded = !expanded)}>
		<p class="text-14 text-bold title">{topic.title}</p>

		<div class="header__details">
			{#if topic.hasIssue}
				<Button size="tag" clickable={false}>Has Issue</Button>
			{/if}

			{#if expanded}
				<Icon name="chevron-down" />
			{:else}
				<Icon name="chevron-up" />
			{/if}
		</div>
	</div>

	{#if expanded}
		<div class="footer">
			<div class="markdown text-13 text-body">
				<Markdown content={topic.body} />
			</div>
			<div class="footer__actions">
				<Button onclick={() => createTopicModal?.open()}>Edit</Button>
				{#if !topic.hasIssue && $gitHost?.issueService()}
					<Button onclick={() => createIssueModal?.open()}>Convert to issue</Button>
				{/if}
				<Button icon="bin" style="error" onclick={() => deleteModal?.show()} />
			</div>
		</div>
	{/if}
</div>

<style lang="postcss">
	.topic {
		border-bottom: 1px solid var(--clr-border-2);

		&:last-child {
			border: none;
		}
	}

	.header {
		display: flex;

		justify-content: space-between;
		align-items: center;

		padding: 16px;
	}

	.header__details {
		display: flex;

		gap: 8px;

		align-items: center;
	}

	.footer {
		border-top: 1px solid var(--clr-border-3);

		width: 100%;

		padding: 16px;
	}

	.footer__actions {
		display: flex;

		justify-content: flex-end;

		width: 100%;

		gap: 8px;
	}
</style>
