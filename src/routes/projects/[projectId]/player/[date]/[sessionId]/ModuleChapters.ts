import { type StateDto, Module, Value } from 'mm-jsr';

export type ModuleChaptersSettings = [number, number][];

export type ChangeChaptersCommand = [number, number][];

/**
 * Module for rendering chapters on the rail.
 * Only workds with single value.
 *
 * Uses `.jsr_chapters` CSS class to render a chapters container.
 * Uses `.jsr_chapter` CSS class to render a chapter.
 * Uses `.jsr_chapter__not-filled` CSS class to render a not-filled part of a chapter.
 * Uses `.jsr_chapter__filled` CSS class to render a filled part of a chapter.
 */
export class ModuleChapters extends Module {
	private container!: HTMLElement;
	private chapters!: HTMLElement[];
	private settings!: ModuleChaptersSettings;

	constructor(settings: ModuleChaptersSettings = []) {
		super();

		this.changeLimit(settings);
		this.chapters = [];
	}

	public destroy() {
		this.container.remove();
	}

	public changeLimit(command: ChangeChaptersCommand) {
		this.settings = command;
	}

	public initView() {
		this.container = document.createElement('ul');
		this.container.classList.add('jsr_chapters');
		this.renderer.addChild(this.container);
		this.container.style.display = 'flex';
		this.container.addEventListener('click', this.handleClick);
	}

	private createChapter([from, to]: [Value, Value]) {
		const chapter = document.createElement('li');
		chapter.classList.add('jsr_chapter');

		const filled = document.createElement('div');
		filled.classList.add('jsr_chapter__filled');
		chapter.appendChild(filled);

		const notFilled = document.createElement('div');
		notFilled.classList.add('jsr_chapter__not-filled');
		chapter.appendChild(notFilled);

		chapter.dataset.from = from.asReal().toString();
		chapter.dataset.to = to.asReal().toString();

		chapter.style.left = `${from.asRatio() * 100}%`;
		chapter.style.width = `calc(${(to.asRatio() - from.asRatio()) * 100}%)`;

		this.container.appendChild(chapter);
		this.chapters.push(chapter);

		return chapter;
	}

	private handleClick = (e: MouseEvent) => {
		this.input.setClosestRatioValue(this.renderer.positionToRelative(e.clientX));
	};

	private getChapter([from, to]: [Value, Value]) {
		const found = this.chapters.find(
			(chapter) =>
				chapter.dataset.from === from.asReal().toString() &&
				chapter.dataset.to === to.asReal().toString()
		);
		return found ?? this.createChapter([from, to]);
	}

	private cleanup() {
		this.chapters.forEach((chapter) => {
			const shouldExist = this.settings.some(
				([from, to]) =>
					chapter.dataset.from === from.toString() && chapter.dataset.to === to.toString()
			);
			if (!shouldExist) {
				chapter.remove();
				this.chapters.splice(this.chapters.indexOf(chapter), 1);
			}
		});
	}

	public render(state: StateDto): VoidFunction {
		const toValue = (num: number) => state.values[0].changeReal(num);
		return () => {
			this.cleanup();

			this.settings.forEach(([from, to]) => {
				const chapter = this.getChapter([toValue(from), toValue(to)]);

				const filled = chapter.querySelector('.jsr_chapter__filled') as HTMLElement;
				const notFilled = chapter.querySelector('.jsr_chapter__not-filled') as HTMLElement;
				const filledWidth = (state.values[0].asReal() - from) / (to - from);
                filled.style.width = `calc(${filledWidth * 100}%)`;
                notFilled.style.width = `calc(${(1 - filledWidth) * 100}%)`;

				const isActive = from <= state.values[0].asReal() && state.values[0].asReal() <= to;
				if (isActive) {
					chapter.classList.add('jsr_chapter--active');
				} else {
					chapter.classList.remove('jsr_chapter--active');
				}
			});
		};
	}
}
