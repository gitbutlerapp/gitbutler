<script>
	import { IconArrowBigLeftFilled, IconArrowBigRightFilled } from '@tabler/icons-svelte';
	let history = window.history;
</script>

<div class="flex items-center justify-center space-x-3 text-zinc-400">
	<button
		class="rounded-md p-2 hover:bg-zinc-700 hover:text-zinc-200"
		title="Go back"
		on:click={() => history.back()}
	>
		<div class="flex h-4 w-4 items-center justify-center">
			<svg
				width="16"
				height="12"
				viewBox="0 0 16 12"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
			>
				<path
					fill-rule="evenodd"
					clip-rule="evenodd"
					d="M14.9998 5H3.41376L6.70676 1.707C7.09776 1.316 7.09776 0.684 6.70676 0.293C6.31576 -0.0979999 5.68376 -0.0979999 5.29276 0.293L0.292762 5.293C-0.0982383 5.684 -0.0982383 6.316 0.292762 6.707L5.29276 11.707C5.48776 11.902 5.74376 12 5.99976 12C6.25576 12 6.51176 11.902 6.70676 11.707C7.09776 11.316 7.09776 10.684 6.70676 10.293L3.41376 7H14.9998C15.5528 7 15.9998 6.552 15.9998 6C15.9998 5.448 15.5528 5 14.9998 5Z"
					fill="#5C5F62"
				/>
			</svg>
		</div>
	</button>
	<button
		class="rounded-md p-2 hover:bg-zinc-700 hover:text-zinc-200"
		title="Go forward"
		on:click={() => history.forward()}
	>
		<div class="flex h-4 w-4 items-center justify-center">
			<svg
				width="16"
				height="12"
				viewBox="0 0 16 12"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
			>
				<path
					fill-rule="evenodd"
					clip-rule="evenodd"
					d="M15.707 5.293L10.707 0.293C10.316 -0.0979999 9.684 -0.0979999 9.293 0.293C8.902 0.684 8.902 1.316 9.293 1.707L12.586 5H1C0.447 5 0 5.448 0 6C0 6.552 0.447 7 1 7H12.586L9.293 10.293C8.902 10.684 8.902 11.316 9.293 11.707C9.488 11.902 9.744 12 10 12C10.256 12 10.512 11.902 10.707 11.707L15.707 6.707C16.098 6.316 16.098 5.684 15.707 5.293Z"
					fill="#5C5F62"
				/>
			</svg>
		</div>
	</button>
</div>
