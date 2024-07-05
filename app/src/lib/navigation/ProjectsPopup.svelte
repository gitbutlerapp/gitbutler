<script lang="ts">
	import { ProjectService } from '$lib/backend/projects';
	import Icon from '$lib/shared/Icon.svelte';
	import ScrollableContainer from '$lib/shared/ScrollableContainer.svelte';
	import { getContext } from '$lib/utils/context';
	import type iconsJson from '$lib/icons/icons.json';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	interface ItemSnippetProps {
		label: string;
		selected?: boolean;
		icon?: string;
		onclick: () => void;
	}

	export let isNavCollapsed: boolean;

	const projectService = getContext(ProjectService);
	const projects = projectService.projects;

	let hidden = true;
	let loading = false;

	export function toggle() {
		hidden = !hidden;
		return !hidden;
	}

	export function hide() {
		hidden = true;
	}
</script>

{#snippet itemSnippet(props: ItemSnippetProps)}
	<button
		disabled={props.selected}
		class="list-item"
		class:selected={props.selected}
		on:click={props.onclick}
	>
		<div class="label text-base-14 text-bold">
			{props.label}
		</div>
		{#if props.icon || props.selected}
			<div class="icon">
				{#if props.icon}
					<Icon name={loading ? 'spinner' : props.icon as keyof typeof iconsJson} />
				{:else}
					<Icon name="tick" />
				{/if}
			</div>
		{/if}
	</button>
{/snippet}

{#if !hidden}
	<div class="popup" class:collapsed={isNavCollapsed}>
		{#if $projects.length > 0}
			<ScrollableContainer maxHeight="20rem">
				<div class="popup__projects">
					{#each $projects as project}
						{@const selected = project.id === $page.params.projectId}
						{@render itemSnippet({
							label: project.title,
							selected,
							icon: selected ? 'tick' : undefined,
							onclick: () => {
								goto(`/${project.id}/`);
								hide();
							}
						})}
					{/each}
				</div>
			</ScrollableContainer>
		{/if}
		<div class="popup__actions">
			{@render itemSnippet({
				label: 'Add new project',
				icon: 'plus',
				onclick: async () => {
					loading = true;
					try {
						await projectService.addProject();
					} finally {
						loading = false;
					}
				}
			})}
		</div>
	</div>
{/if}

<style lang="postcss">
	.popup {
		position: absolute;
		top: 100%;
		z-index: var(--z-floating);
		width: 100%;
		margin-top: 6px;
		border-radius: var(--m, 6px);
		border: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
		/* shadow/s */
		box-shadow: 0px 7px 14px 0px rgba(0, 0, 0, 0.1);
		animation: fadeIn 0.2s ease-out forwards;
	}

	@keyframes fadeIn {
		0% {
			opacity: 0;
			transform: translateY(-6px);
		}
		50% {
			opacity: 1;
		}
		100% {
			opacity: 1;
			transform: translateY(0);
		}
	}
	.popup__actions {
		padding: 8px;
		border-top: 1px solid var(--clr-scale-ntrl-70);
	}
	.popup__projects {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: 8px;
	}

	/* LIST ITEM */
	.list-item {
		display: flex;
		align-items: center;
		color: var(--clr-scale-ntrl-10);
		font-weight: 700;
		padding: 10px 10px;
		justify-content: space-between;
		border-radius: var(--radius-m);
		width: 100%;
		transition: background-color var(--transition-fast);

		&:hover:enabled,
		&:focus:enabled {
			background-color: var(--clr-bg-1-muted);
			& .icon {
				color: var(--clr-scale-ntrl-40);
			}
		}
		&:disabled {
			background-color: var(--clr-bg-2);
			color: var(--clr-text-2);
		}
		& .icon {
			display: flex;
			color: var(--clr-scale-ntrl-50);
		}
		& .label {
			height: 16px;
			text-overflow: ellipsis;
			overflow: hidden;
		}
	}

	/* MODIFIERS */
	.popup.collapsed {
		width: 240px;
	}
</style>
