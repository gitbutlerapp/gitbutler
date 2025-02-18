export class CommitSelection {
	state = $state<string>();

	setSelection(i: string) {
		this.state = i;
	}
}
