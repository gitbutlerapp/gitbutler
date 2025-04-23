<script module lang="ts">
	import Line from '$lib/commitLines/Line.svelte';
	import { LineManager } from '$lib/commitLines/lineManager';
	import {
		type Args,
		defineMeta,
		setTemplate,
		type StoryContext
	} from '@storybook/addon-svelte-csf';
	import type { Author, CommitData } from '$lib/commitLines/types';

	const { Story } = defineMeta({
		title: 'Lane / Commit Lines',
		args: {
			remoteCommits: [],
			localCommits: [],
			localAndRemoteCommits: [],
			integratedCommits: []
		},
		argTypes: {
			remoteCommits: { control: { type: 'object' } },
			localCommits: { control: { type: 'object' } },
			localAndRemoteCommits: { control: { type: 'object' } },
			integratedCommits: { control: { type: 'object' } }
		}
	});
</script>

<script lang="ts">
	setTemplate(template);

	const caleb: Author = {
		email: 'hello@calebowens.com',
		gravatarUrl: 'https://gravatar.com/avatar/f43ef760d895a84ca7bb35ff6f4c6b7c'
	};

	function author() {
		return caleb;
	}

	function commit(): CommitData {
		return {
			id: crypto.randomUUID(),
			title: 'This is a commit',
			author: author()
		};
	}

	function relatedCommit(): CommitData {
		return {
			id: crypto.randomUUID(),
			title: 'This is a commit with relations',
			author: author(),
			relatedRemoteCommit: {
				id: crypto.randomUUID(),
				title: 'This is a related commit',
				author: author()
			}
		};
	}
</script>

{#snippet template({ ...args }: Args<typeof Story>, _context: StoryContext<typeof Story>)}
	{@const lineManager = new LineManager({
		remoteCommits: args.remoteCommits ?? [],
		localCommits: args.localCommits ?? [],
		localAndRemoteCommits: args.localAndRemoteCommits ?? [],
		integratedCommits: args.integratedCommits ?? []
	})}
	{#each args.remoteCommits ?? [] as commit}
		<div class="group">
			<Line line={lineManager.get(commit.id)} />
		</div>
	{/each}

	{#each args.localCommits ?? [] as commit}
		<div class="group">
			<Line line={lineManager.get(commit.id)} />
		</div>
	{/each}

	{#each args.localAndRemoteCommits ?? [] as commit}
		<div class="group">
			<Line line={lineManager.get(commit.id)} />
		</div>
	{/each}

	{#each args.integratedCommits ?? [] as commit}
		<div class="group">
			<Line line={lineManager.get(commit.id)} />
		</div>
	{/each}
{/snippet}

<Story
	name="Same fork point. All populated"
	args={{
		remoteCommits: [commit(), commit()],
		localCommits: [commit(), relatedCommit()],
		localAndRemoteCommits: [commit(), commit()],
		integratedCommits: [commit(), commit()]
	}}
/>

<Story
	name="Same fork point. No locals"
	args={{
		remoteCommits: [commit(), commit()],
		localCommits: [],
		localAndRemoteCommits: [commit(), commit()],
		integratedCommits: [commit(), commit()]
	}}
/>

<Story
	name="Same fork point. No local and remotes"
	args={{
		remoteCommits: [commit(), commit()],
		localCommits: [commit(), relatedCommit()],
		localAndRemoteCommits: [],
		integratedCommits: [commit(), commit()]
	}}
/>

<Story
	name="Same fork point. No local and remotes or integrateds"
	args={{
		remoteCommits: [commit(), commit()],
		localCommits: [commit(), relatedCommit()],
		localAndRemoteCommits: [],
		integratedCommits: []
	}}
/>

<Story
	name="Same fork point. No remote"
	args={{
		remoteCommits: [],
		localCommits: [commit(), relatedCommit()],
		localAndRemoteCommits: [commit()],
		integratedCommits: [commit(), commit()]
	}}
/>

<Story
	name="Different fork point. All populated"
	args={{
		remoteCommits: [commit(), commit()],
		localCommits: [commit(), relatedCommit()],
		localAndRemoteCommits: [],
		integratedCommits: [commit(), commit()]
	}}
/>

<Story
	name="Different fork point. No integrated"
	args={{
		remoteCommits: [commit(), commit()],
		localCommits: [commit(), relatedCommit()],
		localAndRemoteCommits: [],
		integratedCommits: []
	}}
/>

<Story
	name="Different fork point. No local"
	args={{
		remoteCommits: [commit(), commit()],
		localCommits: [],
		localAndRemoteCommits: [],
		integratedCommits: [commit(), relatedCommit(), commit()]
	}}
/>

<Story
	name="Different fork point. No integrated, no remote"
	args={{
		remoteCommits: [],
		localCommits: [commit(), relatedCommit()],
		localAndRemoteCommits: [],
		integratedCommits: []
	}}
/>

<Story
	name="Different fork point. Only remote"
	args={{
		remoteCommits: [commit(), commit()],
		localCommits: [],
		localAndRemoteCommits: [],
		integratedCommits: []
	}}
/>

<style lang="postcss">
	.group {
		height: 68px;
	}
</style>
