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
		animation: blinking 1.5s infinite;
	}

	@keyframes blinking {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.5;
		}
	}
</style>
