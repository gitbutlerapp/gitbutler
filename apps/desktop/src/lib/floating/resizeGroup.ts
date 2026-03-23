type Resizer = {
	resizerId: symbol;
	getValue: () => number;
	setValue: (newValue?: number) => void;
	minValue: number;
	position: number;
};

/**
 * Resize target, and adjust neighbors if necessary.
 *
 * Where there are two resizers in one view, we need an ability to shrink
 * neighboring resizers when squeezed. It is fairly complex behavior, but
 * using resizers becomes more intuitive.
 */
export class ResizeGroup {
	resizers: Resizer[] = [];

	register(resizer: Resizer) {
		this.resizers.push(resizer);
		// TODO: Validate positions are unique.
		this.resizers.sort((a, b) => a.position - b.position);
		return () => {
			this.resizers = this.resizers.filter((r) => r !== resizer);
		};
	}

	/**
	 * Returns 0 unless the target resizer has been offset. Such changes need
	 * to be reported back since `Resizer.svelte` events depend on the
	 * coordinate of the initial click.
	 */
	resize(id: symbol, newValue: number): number {
		const index = this.resizers.findIndex((r) => r.resizerId === id);
		if (index < 0) return 0;

		const resizer = this.resizers[index]!;

		if (newValue >= resizer.minValue) {
			// Above minimum size, no neighbor affected.
			resizer.setValue(newValue);
			return 0;
		}

		resizer.setValue(resizer.minValue);

		// Nothing to do without relevant neighbor.
		if (index === 0) {
			return 0;
		}

		// Difference between requested size and min adjusted size.
		let overflow = resizer.minValue - newValue;

		// We might need to squeeze more than one resizer, so we track the
		// cumulative adjustment.
		let subtracted = 0;

		let j = 1;
		while (overflow && index - j >= 0) {
			// This is currently thought of as neighbor on the left, or above.
			const prev = this.resizers[index - j]!;
			const prevValue = prev.getValue();

			// Amount the resizer can shrink.
			const available = Math.max(prevValue - prev.minValue, 0);

			if (available > overflow) {
				prev.setValue(prevValue - overflow);
				subtracted = overflow;
				break;
			} else {
				prev.setValue(prevValue - available);
				overflow = overflow - available;
				subtracted += available;
			}
			j++;
		}
		return subtracted;
	}
}
