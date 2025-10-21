<script lang="ts">
	import CodegenStatusIcon from '$components/codegen/CodegenStatusIcon.svelte';
	import type { ClaudeTodo } from '$lib/codegen/types';

	type Props = {
		todo: ClaudeTodo;
	};
	const { todo }: Props = $props();
</script>

<div class="todo clr-text-2">
	<CodegenStatusIcon status={todo.status} />
	<p
		class="text-12 text-body"
		class:blinking-text={todo.status === 'in_progress'}
		class:clr-text-1={todo.status === 'pending'}
		class:todo-strikethrough={todo.status === 'completed'}
	>
		{todo.content}
	</p>
</div>

<style lang="post-css">
	.todo {
		display: flex;
		width: 100%;
		gap: 8px;
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
		animation: gradient-sweep 3s ease-in-out infinite;
	}

	@keyframes gradient-sweep {
		to {
			background-position: 300% center;
		}
	}
</style>
