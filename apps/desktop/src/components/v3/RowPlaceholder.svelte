<script lang="ts">
	import { createCommitPath } from '$lib/routes/routes.svelte';
	import { goto } from '$app/navigation';

	interface Props {
		last?: boolean;
		content?: string;
		commitId: string;
		branchName: string;
		projectId: string;
		stackId: string;
	}

	const {
		projectId,
		commitId,
		branchName,
		stackId,
		last,
		content = 'Commit here'
	}: Props = $props();
</script>

<button
	class="row-here"
	type="button"
	class:last
	onclick={() => goto(createCommitPath(projectId, stackId, branchName, commitId))}
>
	<div class="row-here__circle"></div>
	<div class="row-here__line"></div>
	<div class="row-here__label text-11 text-semibold">{content}</div>
</button>

<style>
	.row-here {
		width: 100%;
		position: absolute;
		height: 100%;
		top: -50%;
		display: flex;
		align-items: center;
		opacity: 0;
		z-index: var(--z-lifted);
		&:hover {
			opacity: 1;
		}
		&.last {
			bottom: -50%;
			top: unset;
		}
	}
	.row-here__circle {
		position: absolute;
		left: 16px;
		top: 50%;
		transform: translateY(-50%);
		border-radius: 100%;
		width: 10px;
		height: 10px;
		background-color: var(--clr-theme-pop-element);
		outline: 2px solid var(--clr-bg-2);
	}
	.row-here__line {
		background-color: var(--clr-theme-pop-element);
		height: 2px;
		flex-grow: 1;
		margin-left: -15px;
	}
	.row-here__label {
		position: absolute;
		top: 50%;
		left: 38px;
		transform: translateY(-50%);
		padding: 2px 6px;
		border-radius: var(--radius-ml);
		background-color: var(--clr-theme-pop-element);
		color: var(--clr-core-ntrl-100);
	}
</style>
