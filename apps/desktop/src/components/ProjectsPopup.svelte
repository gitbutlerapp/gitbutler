<script lang="ts">
	import ScrollableContainer from '$components/ScrollableContainer.svelte';
	import { ProjectsService } from '$lib/project/projectsService';
	import { getContext } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { portal } from '@gitbutler/ui/utils/portal';
	import { resizeObserver } from '@gitbutler/ui/utils/resizeObserver';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	interface ItemSnippetProps {
		label: string;
		selected?: boolean;
		icon?: string;
		loading?: boolean;
		onclick: (event?: any) => void;
	}

	interface ProjectsPopupProps {
		target: HTMLButtonElement;
		isNavCollapsed: boolean;
	}

	const { target, isNavCollapsed }: ProjectsPopupProps = $props();

	const projectsService = getContext(ProjectsService);
	const projects = projectsService.projects;

	let inputBoundingRect: DOMRect | undefined = $state();
	let optionsEl: HTMLDivElement | undefined = $state();
	let hidden = $state(true);

	let newProjectLoading = $state(false);
	let cloneProjectLoading = $state(false);

	function getInputBoundingRect() {
		if (target) {
			inputBoundingRect = target.getBoundingClientRect();
		}
	}

	export function show() {
		hidden = false;
		getInputBoundingRect();
	}

	export function hide() {
		hidden = true;
	}

	export function toggle() {
		if (hidden) {
			show();
		} else {
			hide();
		}
	}

	function clickOutside(e: MouseEvent) {
		if (e.target === e.currentTarget) hide();
	}
</script>

{#snippet itemSnippet(props: ItemSnippetProps)}
	<button
		type="button"
		disabled={props.selected}
		class="list-item"
		class:selected={props.selected}
		onclick={props.onclick}
	>
		<div class="label text-14 text-bold">
			{props.label}
		</div>
		{#if props.icon || props.selected}
			<div class="icon">
				{#if props.icon}
					<Icon name={props.loading ? 'spinner' : (props.icon as keyof typeof iconsJson)} />
				{:else}
					<Icon name="tick" />
				{/if}
			</div>
		{/if}
	</button>
{/snippet}

{#if !hidden}
	<div
		role="presentation"
		class="overlay-wrapper"
		use:resizeObserver={() => {
			getInputBoundingRect();
		}}
		onclick={clickOutside}
		use:portal={'body'}
	>
		<div
			bind:this={optionsEl}
			class="popup"
			class:collapsed={isNavCollapsed}
			style:width={!isNavCollapsed ? `${inputBoundingRect?.width}px` : undefined}
			style:top={inputBoundingRect?.top
				? `${inputBoundingRect.top + inputBoundingRect.height}px`
				: undefined}
			style:left={inputBoundingRect?.left ? `${inputBoundingRect.left}px` : undefined}
		>
			{#if $projects && $projects.length > 0}
				<ScrollableContainer maxHeight="20rem">
					<div class="popup__projects">
						{#each $projects as project}
							{@const selected =
								project.id === $page.params.projectId ||
								$projects.some((p) => p.is_open && p.id === project.id)}
							{@render itemSnippet({
								label: project.title,
								selected,
								icon: selected ? 'tick' : undefined,
								onclick: async (event: any) => {
									if (event.altKey) {
										await projectsService.openProjectInNewWindow(project.id);
									} else {
										goto(`/${project.id}/`);
									}
									hide();
								}
							})}
						{/each}
					</div>
				</ScrollableContainer>
			{/if}
			<div class="popup__actions">
				{@render itemSnippet({
					label: 'Add local repository',
					icon: 'plus',
					loading: newProjectLoading,
					onclick: async () => {
						newProjectLoading = true;
						try {
							await projectsService.addProject();
						} finally {
							newProjectLoading = false;
							hide();
						}
					}
				})}
				{@render itemSnippet({
					label: 'Clone repository',
					icon: 'clone',
					loading: cloneProjectLoading,
					onclick: async () => {
						cloneProjectLoading = true;
						try {
							goto('/onboarding/clone');
						} finally {
							cloneProjectLoading = false;
							hide();
						}
					}
				})}
			</div>
		</div>
	</div>
{/if}

<style lang="postcss">
	.overlay-wrapper {
		z-index: var(--z-blocker);
		position: fixed;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
	}

	.popup {
		position: absolute;
		top: 100%;
		z-index: var(--z-floating);
		margin-top: 4px;
		border-radius: var(--m, 6px);
		border: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
		/* shadow/s */
		box-shadow: 0px 7px 14px 0px rgba(0, 0, 0, 0.1);
		animation: fadeIn 0.17s ease-out forwards;
	}

	@keyframes fadeIn {
		0% {
			opacity: 0;
			transform: translateY(-6px);
		}
		40% {
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
		text-align: left;
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
			margin-top: 2px;
			color: var(--clr-scale-ntrl-50);
		}
		& .label {
			line-height: 140%;
		}
	}

	.popup.collapsed {
		width: 240px;
	}
</style>
