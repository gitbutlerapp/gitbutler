<script lang="ts">
	import { getTimeAgo } from '$lib/utils/timeAgo';
	import { createEventDispatcher } from 'svelte';
	import type { Commit, RemoteCommit } from '$lib/vbranches/types';

	// Overview Details
	export let hash: string;
	export let createdAt: Date;
	export let title: string;
	export let description: string;
	export let author: string;

	export let syncStatus: 'local' | 'localAndRemote' | 'upstream';

	// List positioning
	export let top: boolean;
	export let bottom: boolean;

	// Drag and drop support
	export let acceptAmmend: (commit: Commit | RemoteCommit) => boolean;
	export let acceptSquash: (commit: Commit | RemoteCommit) => boolean;

	let open = false;

	interface Events {
		amend: { commit: Commit | RemoteCommit };
		squash: { commit: Commit | RemoteCommit };
	}

	const dispatcher = createEventDispatcher<Events>();

	const accentColors = {
		local: 'var(--clr-commit-local)',
		localAndRemote: 'var(--clr-commit-remote)',
		upstream: 'var(--clr-commit-upstream)'
	};
	const accentColor = accentColors[syncStatus];
</script>

<div class="commit" class:top class:bottom>
	<div class="accent" style="--accent-color: {accentColor}"></div>
	<div class="content">
		<div class="head">
			<!-- TODO: Consider how we want to handle the top commit <p>Local and remote</p> -->
			<p class="text-base-15 text-bold">{title}</p>
			<div class="head-details">
				<p>
					{hash}
					<span class="details-divider">•</span>
					{getTimeAgo(createdAt)}
					{author}
				</p>
			</div>
		</div>
		<slot name="actions"></slot>
		<slot name="files"></slot>
	</div>
</div>

<style lang="postcss">
	:root {
		--accent-size: var(--size-4);
		--padding-size: var(--size-14);
	}

	.commit {
		display: grid;
		grid-template-columns: var(--accent-size) 1fr;

		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		border-left: none;
		border-radius: var(--radius-m);

		overflow: hidden;

		&.top {
			border-bottom-left-radius: 0;
			border-bottom-right-radius: 0;
		}

		&.bottom {
			border-top-left-radius: 0;
			border-top-right-radius: 0;
		}
	}

	.accent {
		height: 100%;
		background-color: var(--accent-color);
	}

	.content {
		padding: var(--padding-size);
		/* Offset the accent */
		padding-left: calc(var(--padding-size) - var(--accent-size));
	}
</style>
