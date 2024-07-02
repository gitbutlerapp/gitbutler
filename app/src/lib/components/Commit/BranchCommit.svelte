<script lang="ts">
	import BranchFilesList from '$lib/components/BranchFilesList.svelte';
	import Commit from '$lib/components/Commit/Commit.svelte';
	import Tag from '$lib/components/Tag.svelte';
	import type { AnyFile } from '$lib/vbranches/types';

	// This is the point of integration for a commit under a branch
	let files: AnyFile[] = [];
	const isUnapplied = false;
</script>

<Commit
	syncStatus="upstream"
	hash={'123FJE'}
	createdAt={new Date()}
	title={'This is a gud commit'}
	description={'Made stuff hehehe'}
	author={'Caleb Owens'}
>
	<svelte:fragment slot="actions">
		<Tag clickable kind="solid" style="ghost">Undo</Tag>
		<Tag clickable kind="solid" style="ghost">Ammend Message</Tag>
	</svelte:fragment>
	<svelte:fragment slot="files"></svelte:fragment>
</Commit>
