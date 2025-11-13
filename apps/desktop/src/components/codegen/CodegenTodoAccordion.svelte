<script lang="ts">
	import CodegenStatusIcon from '$components/codegen/CodegenStatusIcon.svelte';
	import { Icon } from '@gitbutler/ui';
	import type { ClaudeTodo } from '$lib/codegen/types';

	type Props = {
		todos: ClaudeTodo[];
	};
	const { todos }: Props = $props();

	const displayTodo = $derived(
		[...todos].reverse().find((todo) => todo.status === 'in_progress') ?? todos[0]
	);

	const completedCount = $derived(todos.filter((todo) => todo.status === 'completed').length);
	const totalCount = $derived(todos.length);

	let expanded = $state(false);
</script>

<div class="todos-container">
	<button type="button" class="todo-header" onclick={() => (expanded = !expanded)}>
		<div class="todo-header__chevron" class:expanded>
			<Icon name="chevron-right" />
		</div>
		{#if displayTodo && !expanded}
			<CodegenStatusIcon status={displayTodo.status} />
			<span class="text-12 clr-text-2 truncate">
				{completedCount}/{totalCount}.
				{displayTodo.content}
			</span>
		{:else}
			<span class="text-13 text-bold">{completedCount}/{totalCount}. Todos</span>
		{/if}
	</button>

	{#if expanded}
		<div class="todos-list">
			{#each todos as todo}
				<div class="todo clr-text-2">
					<div class="todo-icon">
						<CodegenStatusIcon status={todo.status} />
					</div>
					<p
						class="text-12 text-body"
						class:blinking-text={todo.status === 'in_progress'}
						class:clr-text-1={todo.status === 'pending'}
						class:todo-strikethrough={todo.status === 'completed'}
					>
						{todo.content}
					</p>
				</div>
			{/each}
		</div>
	{/if}
</div>

<style lang="post-css">
	.todos-container {
		display: flex;
		flex-direction: column;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.todo-header {
		display: flex;
		align-items: center;
		padding: 10px;
		gap: 8px;
		transition: background-color var(--transition-fast);

		&:hover {
			/* background-color: var(--clr-bg-1-muted); */

			& .todo-header__chevron {
				color: var(--clr-text-2);
			}
		}
	}

	.todo-header__chevron {
		display: flex;
		color: var(--clr-text-3);
		transition:
			transform var(--transition-medium),
			color var(--transition-fast);

		&.expanded {
			transform: rotate(90deg);
		}
	}

	.todos-list {
		display: flex;
		flex-direction: column;
		padding: 4px 12px 16px 12px;
		gap: 8px;
	}

	.todo {
		display: flex;
		align-items: flex-start;
		width: 100%;
		gap: 8px;
	}

	.todo-icon {
		display: flex;
		flex-shrink: 0;
		margin-top: 3px;
	}

	.todo-strikethrough {
		text-decoration: line-through;
	}

	.blinking-text {
		background: linear-gradient(
			90deg,
			var(--clr-text-2) 0%,
			var(--clr-text-2) 50%,
			color-mix(in srgb, var(--clr-text-2), transparent 70%) 60%,
			var(--clr-text-2) 100%
		);
		background-size: 300% 100%;
		background-clip: text;
		-webkit-background-clip: text;
		/* opacity: 0.6; */
		-webkit-text-fill-color: transparent;
		animation: gradient-sweep 4.5s ease-in-out infinite;
	}

	@keyframes gradient-sweep {
		to {
			background-position: 300% center;
		}
	}
</style>
