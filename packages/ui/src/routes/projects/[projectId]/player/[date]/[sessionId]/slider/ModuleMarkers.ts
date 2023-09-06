import { type StateDto, Module, Value } from 'mm-jsr';

export type Marker = {
	value: number;
	large: boolean;
};

export type ModuleMarkersSettings = Marker[];

export type ChangeMarkersCommand = Marker[];

/**
 * Module for rendering markers on the rail.
 * Only works with single value.
 *
 * Uses `.jsr_marker` CSS class to render a marker.
 */
export class ModuleMarkers extends Module {
	private settings!: ModuleMarkersSettings;

	constructor(settings: ModuleMarkersSettings = []) {
		super();

		this.settings = settings;
	}

	public setLarge(value: number, large: boolean) {
		const marker = this.settings.find((marker) => marker.value === value);
		if (marker && marker?.large !== large) {
			marker.large = large;
			this.renderer
				.getContainer()
				.querySelector(`.jsr_marker[data-value="${value}"]`)
				?.classList.toggle('jsr_marker--large', large);
		}
	}

	public destroy() {
		this.renderer
			.getContainer()
			.querySelectorAll('.jsr_marker')
			.forEach((marker) => marker.remove());
	}

	private createMarker(value: Value) {
		const marker = document.createElement('div');
		marker.classList.add('jsr_marker');
		marker.dataset.value = value.asReal().toString();
		marker.style.left = `${value.asRatio() * 100}%`;
		marker.addEventListener('click', () => this.input.setRealValue(0, value.asReal()));
		this.renderer.addChild(marker);
		return marker;
	}

	private getMarker(value: Value) {
		return (
			this.renderer.getContainer().querySelector(`.jsr_marker[data-value="${value.asReal()}"]`) ||
			this.createMarker(value)
		);
	}

	private cleanup() {
		const markers = this.renderer.getContainer().querySelectorAll('.jsr_marker');
		markers.forEach((marker) => {
			const shouldExist = this.settings.some(
				(value) => value.value.toString() === (marker as HTMLElement).dataset.value
			);
			if (!shouldExist) {
				marker.remove();
			}
		});
	}

	public render(state: StateDto): VoidFunction {
		const toValue = (num: number) => state.values[0].changeReal(num);
		return () => {
			this.cleanup();

			this.settings.forEach((value) => {
				const marker = this.getMarker(toValue(value.value));
				marker.classList.toggle('jsr_marker--after', value.value > state.values[0].asReal());
				marker.classList.toggle('jsr_marker--large', value.large);
			});
		};
	}
}
