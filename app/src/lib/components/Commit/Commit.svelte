<script lang="ts">
	import { createEventDispatcher } from 'svelte';
	import type { Commit, RemoteCommit } from '$lib/vbranches/types';

	export let acceptAmmend: (commit: Commit | RemoteCommit) => boolean;
	export let acceptSquash: (commit: Commit | RemoteCommit) => boolean;
	export let local: boolean;
	export let remote: boolean;
	export let hash: string;
	export let createdAt: Date;
	export let title: string;
	export let description: string;
	export let borderBottom: boolean;

	interface Events {
		amend: { commit: Commit | RemoteCommit };
		squash: { commit: Commit | RemoteCommit };
	}

	const dispatcher = createEventDispatcher<Events>();
</script>

<slot name="actions"></slot>
<slot name="files"></slot>
