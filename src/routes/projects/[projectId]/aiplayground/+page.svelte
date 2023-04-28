<script lang="ts">
	import Button from '$lib/components/Button/Button.svelte';
	import { onMount } from 'svelte';

	const debounce = <T extends (...args: any[]) => any>(fn: T, delay: number) => {
		let timeout: ReturnType<typeof setTimeout>;
		return (...args: any[]) => {
			clearTimeout(timeout);
			timeout = setTimeout(() => fn(...args), delay);
		};
	};

	// const chainUrl = 'http://127.0.0.1:8000';
	const chainUrl = 'https://zpuszumgur.us-east-1.awsapprunner.com';

	async function createSummary(text: string) {
		const response = await fetch(`${chainUrl}/summaries`, {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json'
			},
			body: JSON.stringify({ text: text })
		});

		const data = await response.json();
		return data;
	}

	async function createChat() {
		const response = await fetch(`${chainUrl}/chats`, {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json'
			},
			body: JSON.stringify({})
		});

		const data = await response.json();
		return data;
	}

	async function getChatHistory(id: string) {
		const response = await fetch(`${chainUrl}/chats/${id}`, {
			method: 'GET'
		});

		const data = await response.json();
		return data;
	}

	async function newChatMessage(id: string, text: string) {
		const response = await fetch(`${chainUrl}/chats`, {
			method: 'PATCH',
			headers: {
				'Content-Type': 'application/json'
			},
			body: JSON.stringify({ id, text: text })
		});

		const sequence = await response.json();
		return sequence;
	}

	async function addToSummary(id: string, newText: string) {
		const response = await fetch(`${chainUrl}/summaries`, {
			method: 'PATCH',
			headers: {
				'Content-Type': 'application/json'
			},
			body: JSON.stringify({ id, text: newText })
		});

		const sequence = await response.json();
		return sequence;
	}

	async function getSummary(id: string) {
		const response = await fetch(`${chainUrl}/summaries/${id}`, {
			method: 'GET'
		});

		const data = await response.json();
		return data;
	}

	let summaryId = '';
	let input = '';
	let sequence = 0;
	let processedSeq = 0;
	let summary = '';

	$: if (summaryId && processedSeq != sequence) {
		debounce(() => {
			console.log('polling summary');
			getSummary(summaryId).then((data) => {
				summary = data.text;
				processedSeq = data.sequence;
			});
		}, 1000)();
	}

	const setupChat = () => {
		createChat().then((data) => {
			chatId = data.id;
		});
		chatHistory = [];
		chatInput = '';
	};

	let chatId = '';
	let chatHistory: string[][] = [];
	let chatInput = '';
	let completedSeq = 0;
	let requestedSeq = 0;

	onMount(setupChat);

	let ms = 2000;
	let clear: any;
	$: if (chatId) {
		clearInterval(clear);
		clear = setInterval(() => {
			getChatHistory(chatId).then((data) => {
				chatHistory = data.history;
				completedSeq = data.sequence;
				console.log(chatHistory);
			});
		}, ms);
	}

	$: waitingForResponse = requestedSeq != completedSeq;
</script>

<div class="GitBTLR-container  h-full p-4">
	<div class="relative flex h-full flex-col rounded border border-zinc-700 bg-card-default shadow">
		<div class="flex  justify-between gap-2 border-b border-zinc-700 bg-card-active p-2">
			<div class="flex gap-2 text-xl">Chat GitButler</div>

			<div class="flex items-center gap-2">
				<Button role="basic" height="small" on:click={setupChat}>Reset chat</Button>
			</div>
		</div>

		<div class="chat-container flex h-full flex-col overflow-auto border-zinc-700 pb-[122px]">
			<ul class="flex flex-col gap-2 p-4">
				<div class="flex items-start gap-2 align-top">
					<div class="chat-user-avatar bg-zinc-900">
						<svg
							width="20"
							height="20"
							viewBox="0 0 20 20"
							fill="none"
							xmlns="http://www.w3.org/2000/svg"
						>
							<path
								d="M14.5614 12.4668C14.3108 12.3265 14.0786 12.1555 13.8712 11.9587C13.4477 11.5571 13.1091 11.0403 12.8663 10.4225L12.1828 8.68652L11.4994 10.4225C11.2561 11.0403 10.918 11.5571 10.4945 11.9587C10.2861 12.1564 10.0539 12.3274 9.80476 12.4673L8.66333 13.108L9.80476 13.7482C10.0539 13.888 10.2861 14.059 10.494 14.2567C10.918 14.6583 11.2561 15.1757 11.4989 15.7929L12.1823 17.5289L12.8658 15.7929C13.1091 15.1751 13.4472 14.6583 13.8707 14.2567C14.0791 14.059 14.3113 13.8875 14.5609 13.7482L15.7029 13.1075L14.5614 12.4668Z"
								fill="#F4F4F5"
							/>
							<path
								d="M11.9573 4.05509C11.7746 3.95585 11.6049 3.83459 11.4532 3.69517C11.1469 3.41352 10.9033 3.05324 10.7293 2.62445L10.4759 2L10.2226 2.62445C10.0486 3.05322 9.80498 3.41349 9.49868 3.69517C9.34626 3.83529 9.1769 3.95655 8.99493 4.05544L8.57153 4.28572L8.99493 4.516C9.1769 4.6149 9.34626 4.73615 9.49868 4.87627C9.80497 5.15757 10.0486 5.51784 10.2226 5.94699L10.4759 6.57143L10.7293 5.94699C10.9033 5.51823 11.1469 5.15795 11.4532 4.87627C11.6056 4.73615 11.775 4.61489 11.9573 4.516L12.3811 4.28537L11.9573 4.05509Z"
								fill="#F4F4F5"
							/>
							<path
								d="M8.56381 8.66611C8.31754 8.52816 8.08878 8.35963 7.88429 8.16583C7.47143 7.77434 7.14309 7.27356 6.90848 6.67755L6.56702 5.80957L6.22556 6.67755C5.99096 7.27353 5.66262 7.7743 5.24974 8.16583C5.04429 8.3606 4.816 8.52915 4.57071 8.6666L4 8.98668L4.57071 9.30677C4.81601 9.44423 5.04429 9.61277 5.24974 9.80754C5.66261 10.1985 5.99094 10.6993 6.22556 11.2958L6.56702 12.1638L6.90848 11.2958C7.14308 10.6999 7.47142 10.1991 7.88429 9.80754C8.08975 9.61277 8.31804 9.44422 8.56381 9.30677L9.13502 8.9862L8.56381 8.66611Z"
								fill="#F4F4F5"
							/>
						</svg>
					</div>
					<div class="message-block flex flex-col gap-2">
						<div class="automated-message">
							<div class="automated-text">
								Hello! I can questions specific to the code or history of your codebase. You can ask me things like "How/Where is use authentication implemented?" and "What's the story behind the bookmarking feature?"
							</div>
						</div>
					</div>
				</div>

				{#if waitingForResponse}
					<!-- Generating response...  -->
					<div class="flex items-start gap-2 align-top">
						<div class="chat-user-avatar bg-zinc-900">
							<div class="loading-orbit" />
							<svg
								width="20"
								height="20"
								viewBox="0 0 20 20"
								class="h-4 w-4"
								style="z-index: 300;"
								fill="none"
								xmlns="http://www.w3.org/2000/svg"
							>
								<path
									d="M14.5614 12.4668C14.3108 12.3265 14.0786 12.1555 13.8712 11.9587C13.4477 11.5571 13.1091 11.0403 12.8663 10.4225L12.1828 8.68652L11.4994 10.4225C11.2561 11.0403 10.918 11.5571 10.4945 11.9587C10.2861 12.1564 10.0539 12.3274 9.80476 12.4673L8.66333 13.108L9.80476 13.7482C10.0539 13.888 10.2861 14.059 10.494 14.2567C10.918 14.6583 11.2561 15.1757 11.4989 15.7929L12.1823 17.5289L12.8658 15.7929C13.1091 15.1751 13.4472 14.6583 13.8707 14.2567C14.0791 14.059 14.3113 13.8875 14.5609 13.7482L15.7029 13.1075L14.5614 12.4668Z"
									fill="#F4F4F5"
								/>
								<path
									d="M11.9573 4.05509C11.7746 3.95585 11.6049 3.83459 11.4532 3.69517C11.1469 3.41352 10.9033 3.05324 10.7293 2.62445L10.4759 2L10.2226 2.62445C10.0486 3.05322 9.80498 3.41349 9.49868 3.69517C9.34626 3.83529 9.1769 3.95655 8.99493 4.05544L8.57153 4.28572L8.99493 4.516C9.1769 4.6149 9.34626 4.73615 9.49868 4.87627C9.80497 5.15757 10.0486 5.51784 10.2226 5.94699L10.4759 6.57143L10.7293 5.94699C10.9033 5.51823 11.1469 5.15795 11.4532 4.87627C11.6056 4.73615 11.775 4.61489 11.9573 4.516L12.3811 4.28537L11.9573 4.05509Z"
									fill="#F4F4F5"
								/>
								<path
									d="M8.56381 8.66611C8.31754 8.52816 8.08878 8.35963 7.88429 8.16583C7.47143 7.77434 7.14309 7.27356 6.90848 6.67755L6.56702 5.80957L6.22556 6.67755C5.99096 7.27353 5.66262 7.7743 5.24974 8.16583C5.04429 8.3606 4.816 8.52915 4.57071 8.6666L4 8.98668L4.57071 9.30677C4.81601 9.44423 5.04429 9.61277 5.24974 9.80754C5.66261 10.1985 5.99094 10.6993 6.22556 11.2958L6.56702 12.1638L6.90848 11.2958C7.14308 10.6999 7.47142 10.1991 7.88429 9.80754C8.08975 9.61277 8.31804 9.44422 8.56381 9.30677L9.13502 8.9862L8.56381 8.66611Z"
									fill="#F4F4F5"
								/>
							</svg>
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

				{#each chatHistory as pair}
					<div class="flex justify-end">
						<div class="user-message ">
							{pair[0]}
						</div>
					</div>
					<div class="flex items-start gap-2 align-top">
						<div class="chat-user-avatar bg-zinc-900">
							<svg
								width="20"
								height="20"
								viewBox="0 0 20 20"
								fill="none"
								xmlns="http://www.w3.org/2000/svg"
							>
								<path
									d="M14.5614 12.4668C14.3108 12.3265 14.0786 12.1555 13.8712 11.9587C13.4477 11.5571 13.1091 11.0403 12.8663 10.4225L12.1828 8.68652L11.4994 10.4225C11.2561 11.0403 10.918 11.5571 10.4945 11.9587C10.2861 12.1564 10.0539 12.3274 9.80476 12.4673L8.66333 13.108L9.80476 13.7482C10.0539 13.888 10.2861 14.059 10.494 14.2567C10.918 14.6583 11.2561 15.1757 11.4989 15.7929L12.1823 17.5289L12.8658 15.7929C13.1091 15.1751 13.4472 14.6583 13.8707 14.2567C14.0791 14.059 14.3113 13.8875 14.5609 13.7482L15.7029 13.1075L14.5614 12.4668Z"
									fill="#F4F4F5"
								/>
								<path
									d="M11.9573 4.05509C11.7746 3.95585 11.6049 3.83459 11.4532 3.69517C11.1469 3.41352 10.9033 3.05324 10.7293 2.62445L10.4759 2L10.2226 2.62445C10.0486 3.05322 9.80498 3.41349 9.49868 3.69517C9.34626 3.83529 9.1769 3.95655 8.99493 4.05544L8.57153 4.28572L8.99493 4.516C9.1769 4.6149 9.34626 4.73615 9.49868 4.87627C9.80497 5.15757 10.0486 5.51784 10.2226 5.94699L10.4759 6.57143L10.7293 5.94699C10.9033 5.51823 11.1469 5.15795 11.4532 4.87627C11.6056 4.73615 11.775 4.61489 11.9573 4.516L12.3811 4.28537L11.9573 4.05509Z"
									fill="#F4F4F5"
								/>
								<path
									d="M8.56381 8.66611C8.31754 8.52816 8.08878 8.35963 7.88429 8.16583C7.47143 7.77434 7.14309 7.27356 6.90848 6.67755L6.56702 5.80957L6.22556 6.67755C5.99096 7.27353 5.66262 7.7743 5.24974 8.16583C5.04429 8.3606 4.816 8.52915 4.57071 8.6666L4 8.98668L4.57071 9.30677C4.81601 9.44423 5.04429 9.61277 5.24974 9.80754C5.66261 10.1985 5.99094 10.6993 6.22556 11.2958L6.56702 12.1638L6.90848 11.2958C7.14308 10.6999 7.47142 10.1991 7.88429 9.80754C8.08975 9.61277 8.31804 9.44422 8.56381 9.30677L9.13502 8.9862L8.56381 8.66611Z"
									fill="#F4F4F5"
								/>
							</svg>
						</div>
						<div class="message-block flex max-w-[500px] flex-col gap-2">
							<div class="automated-message">
								<div class="automated-text">{pair[1]}</div>
							</div>
						</div>
					</div>
				{/each}
			</ul>
		</div>

		<div
			class="absolute bottom-0 flex w-full flex-col gap-2 border-t border-zinc-700 p-4 "
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
					class="w-full"
				/>
				<Button
					disabled={chatInput.length == 0 || !chatId}
					role="primary"
					on:click={() => {
						newChatMessage(chatId, chatInput).then((data) => {
							requestedSeq = +data;
							chatInput = '';
						});
					}}
				>
					Send
				</Button>
			</div>
			<div class="flex">
				<p class="text-sm text-zinc-500">
					Chat GitButler may produce inaccurate statements about your code. Please use your
					judgement before you commit changes.
				</p>
			</div>
		</div>
	</div>
</div>

<style>
	.chat-user-avatar {
		@apply relative flex h-[40px] w-[40px] items-center justify-center rounded-full p-2;
	}

	.automated-message {
		@apply max-w-[500px] rounded-[18px] rounded-tl-md bg-zinc-200 text-[14px] font-medium text-zinc-800;
	}
	.automated-text {
		@apply w-full cursor-text select-text px-4 py-3 text-[14px];
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

	.breathing-orb {
		/* Styling */
		position: absolute;
		width: 20px;
		height: 20px;
		left: 16px;
		top: 6px;
		animation: breathingOrb 4s ease-in-out infinite;

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
	}

	/*
	* Set up the keyframes to actually describe the begining and end states of 
	* the animation.  The browser will interpolate all the frames between these 
	* points.  Again, remember your vendor-specific prefixes for now!
	*/
	@keyframes breathingOrb {
		0% {
			opacity: 0.8;
		}
		50% {
			opacity: 0.4;
		}
		100% {
			opacity: 0.8;
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
