<script lang="ts">
	import { Button, Link } from '$lib/components';
	import { CloudApi, type Project } from '$lib/api';
	import { stores } from '$lib';
	import { marked } from 'marked';
	import { IconAISparkles } from '$lib/icons';

	const cloud = CloudApi();
	const user = stores.user;

	export let project: Project;

	let chatId = '';
	let chatHistory: string[][] = [];
	let chatInput = '';
	let completedSeq = 0;
	let requestedSeq = 0;
	$: waitingForResponse = requestedSeq != completedSeq;

	$: if (!waitingForResponse) {
		chatInput = '';
	}

	$: cloudEnabled = !!project?.api?.repository_id;

	let ms = 3000;
	let clear: any;
	$: if (chatId) {
		clearInterval(clear);
		clear = setInterval(() => {
			cloud.chat
				.history($user?.access_token || '', project?.api?.repository_id || '', chatId)
				.then((data) => {
					chatHistory = data.history;
					completedSeq = data.sequence;
				});
		}, ms);
	}
</script>

<div class="GitBTLR-container  h-full p-4">
	<div class="card relative flex h-full flex-col">
		<div class="flex  justify-between gap-2 border-b border-zinc-700 bg-card-active p-2">
			<div class="flex gap-2 text-xl">Codebase knowledgebase</div>
		</div>

		<div class="chat-container flex h-full flex-col overflow-auto border-zinc-700 pb-[122px]">
			<ul class="flex flex-col gap-2 p-4">
				{#if cloudEnabled}
					<div class="flex items-start gap-2 align-top">
						<div class="chat-user-avatar bg-zinc-900">
							<IconAISparkles />
						</div>
						<div class="message-block flex flex-col gap-2">
							<div class="automated-message">
								<div class="automated-text">
									Hello! I can questions specific to the code or history of your codebase. You can
									ask me things like "How/Where is use authentication implemented?" and "What's the
									story behind the bookmarking feature?"
								</div>
							</div>
						</div>
					</div>

					{#each chatHistory as pair}
						<div class="flex justify-end">
							<div class="user-message ">
								{pair[0]}
							</div>
						</div>
						<div class="flex items-start gap-2 align-top">
							<div class="chat-user-avatar bg-zinc-900">
								<IconAISparkles />
							</div>
							<div class="message-block flex max-w-[500px] flex-col gap-2">
								<div class="automated-message">
									<div class="automated-text">{@html marked(pair[1])}</div>
								</div>
							</div>
						</div>
					{/each}

					{#if waitingForResponse}
						<!-- Generating response...  -->
						<div class="flex items-start gap-2 align-top">
							<div class="chat-user-avatar bg-zinc-900">
								<div class="loading-orbit" />
								<IconAISparkles />
							</div>
							<div class="message-block flex flex-col gap-2">
								<div class="automated-message">
									<div class="automated-text">
										<span class="dot-container">
											<div class="dot" />
											<div class="dot" />
											<div class="dot" />
										</span>
									</div>
								</div>
							</div>
						</div>

						<!-- END Generating response! -->
					{/if}
				{:else}
					<div class="flex items-start gap-2 align-top">
						<div class="chat-user-avatar bg-zinc-900">
							<IconAISparkles />
						</div>
						<div class="message-block flex flex-col gap-2">
							<div class="automated-message">
								<div class="automated-text">
									To use this feature, you need to have GitButler Cloud enabled. You can do this in
									the
									<a class="cursor-pointer underline" href="/projects/{project?.id}/settings"
										>project settings</a
									>.
								</div>
							</div>
						</div>
					</div>
				{/if}
			</ul>
		</div>

		<div
			class="absolute bottom-0 flex w-full flex-col gap-2 rounded-br rounded-bl border-t border-zinc-700 p-4"
			style="                
				border-width: 0.5px; 
				-webkit-backdrop-filter: blur(5px) saturate(190%) contrast(70%) brightness(80%);
				backdrop-filter: blur(5px) saturate(190%) contrast(70%) brightness(80%);
				background-color: rgba(24, 24, 27, 0.60);
				border: 0.5px solid rgba(63, 63, 70, 0.50);
			"
		>
			<div class="flex gap-2 pt-2">
				<input
					type="text"
					autocomplete="off"
					autocorrect="off"
					spellcheck="true"
					bind:value={chatInput}
					placeholder="Send a message..."
					class="chat-input-text w-full"
					disabled={waitingForResponse}
				/>
				<Button
					disabled={chatInput.length == 0 || waitingForResponse}
					color="primary"
					on:click={() => {
						if (!chatId) {
							cloud.chat
								.new($user?.access_token || '', project.api?.repository_id || '')
								.then((data) => {
									chatId = data.id;
									cloud.chat
										.newMessage(
											$user?.access_token || '',
											project?.api?.repository_id || '',
											chatId,
											chatInput
										)
										.then((data) => {
											requestedSeq = +data;
										});
								});
						} else {
							cloud.chat
								.newMessage(
									$user?.access_token || '',
									project?.api?.repository_id || '',
									chatId,
									chatInput
								)
								.then((data) => {
									requestedSeq = +data;
								});
						}
					}}
				>
					Send
				</Button>
			</div>
		</div>
	</div>
</div>

<style lang="postcss">
	.chat-user-avatar {
		@apply relative flex h-[40px] w-[40px] items-center justify-center rounded-full p-2;
	}

	.automated-message {
		@apply max-w-[500px] rounded-[18px] rounded-tl-md bg-zinc-200 text-[14px] font-medium text-zinc-800;
	}
	.automated-text {
		@apply w-64 cursor-text select-text px-4 py-3 text-[14px];
	}
	.automated-text :global(pre) {
		@apply bg-zinc-300;
		@apply my-2 overflow-x-auto p-2;
	}
	.automated-text :global(code) {
		@apply bg-zinc-300;
	}
	.user-message {
		@apply w-[fit-content] max-w-[80%] cursor-text select-text rounded-[18px] rounded-tr-md bg-blue-700 py-3 px-4 text-[14px] font-medium text-white;
	}

	/**
	* ==============================================
	* Dot Typing
	* ==============================================
	*/
	.dot-container {
		padding-left: 4px;
		padding-bottom: 3px;
	}
	.dot {
		background-color: purple;
		display: inline-block;
		width: 3px;
		height: 3px;
		border-radius: 50%;
		position: relative;
		bottom: 3px;
	}

	.dot-container .dot:nth-last-child(1) {
		animation: jumpingAnimation 1.2s 0.6s linear infinite;
	}
	.dot-container .dot:nth-last-child(2) {
		animation: jumpingAnimation 1.2s 0.3s linear infinite;
	}
	.dot-container .dot:nth-last-child(3) {
		animation: jumpingAnimation 1.2s 0s linear infinite;
	}

	@keyframes jumpingAnimation {
		0% {
			transform: translate(0, 0);
		}
		16% {
			transform: translate(0, -5px);
		}
		33% {
			transform: translate(0, 0);
		}
		100% {
			transform: translate(0, 0);
		}
	}

	.loading-orbit {
		/* Styling */
		position: absolute;
		width: 14px;
		height: 14px;
		left: 16px;
		top: 6px;

		background: rgba(154, 115, 221, 1);
		filter: blur(6px);
		border-radius: 32px;

		/* 
		* Make the initial position to be the center of the circle you want this
		* object follow.
		*/
		position: absolute;
		left: 10px;
		top: 10px;

		/*
		* Sets up the animation duration, timing-function (or easing)
		* and iteration-count. Ensure you use the appropriate vendor-specific 
		* prefixes as well as the official syntax for now. Remember, tools like 
		* CSS Please are your friends!
		*/
		-webkit-animation: loadingOrbit 3s linear infinite; /* Chrome, Safari 5 */
		-moz-animation: loadingOrbit 3s linear infinite; /* Firefox 5-15 */
		-o-animation: loadingOrbit 3s linear infinite; /* Opera 12+ */
		animation: loadingOrbit 3s linear infinite; /* Chrome, Firefox 16+, 
														IE 10+, Safari 5 */
	}

	/*
	* Set up the keyframes to actually describe the begining and end states of 
	* the animation.  The browser will interpolate all the frames between these 
	* points.  Again, remember your vendor-specific prefixes for now!
	*/
	@-webkit-keyframes loadingOrbit {
		0% {
			opacity: 1;
			-webkit-transform: rotate(0deg) translateX(15px) rotate(0deg);
		}
		50% {
			opacity: 0.5;
		}
		100% {
			opacity: 1;
			-webkit-transform: rotate(360deg) translateX(15px) rotate(-360deg);
		}
	}

	@-moz-keyframes loadingOrbit {
		0% {
			opacity: 1;
			-moz-transform: rotate(0deg) translateX(15px) rotate(0deg);
		}
		50% {
			opacity: 0.5;
		}
		100% {
			opacity: 1;
			-moz-transform: rotate(360deg) translateX(15px) rotate(-360deg);
		}
	}

	@-o-keyframes loadingOrbit {
		0% {
			opacity: 1;
			-o-transform: rotate(0deg) translateX(15px) rotate(0deg);
		}
		50% {
			opacity: 0.5;
		}
		100% {
			opacity: 1;
			-o-transform: rotate(360deg) translateX(15px) rotate(-360deg);
		}
	}

	@keyframes loadingOrbit {
		0% {
			opacity: 1;
			transform: rotate(0deg) translateX(15px) rotate(0deg);
		}
		50% {
			opacity: 0.5;
		}
		100% {
			opacity: 1;
			transform: rotate(360deg) translateX(15px) rotate(-360deg);
		}
	}
</style>
