export function playSound(soundUrl: string) {
	const audio = new Audio(soundUrl);
	audio.play();
}
