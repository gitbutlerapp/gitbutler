<script>
	import BranchFilesList from '$lib/components/BranchFilesList.svelte';
	import Commit from '$lib/components/Commit/Commit.svelte';
	import Tag from '$lib/components/Tag.svelte';

	// This is the point of integration for a commit under a branch
</script>

<Commit
	local={true}
	remote={false}
	hash={'123FJE'}
	createdAt={new Date()}
	title={'This is a gud commit'}
	description={'Made stuff hehehe'}
	borderBottom={true}
>
	<svelte:fragment slot="actions">
		<Tag clickable kind="solid" style="ghost">Undo</Tag>
		<Tag clickable kind="solid" style="ghost">Ammend Message</Tag>
	</svelte:fragment>
	<svelte:fragment slot="files">
		<BranchFilesList title="Files" {files} {isUnapplied} />
	</svelte:fragment>
</Commit>
