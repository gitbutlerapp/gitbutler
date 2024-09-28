import { SectionType } from '$lib/utils/fileSections';

export interface Row {
	beforeLineNumber?: number;
	afterLineNumber?: number;
	tokens: string[];
	type: SectionType;
	size: number;
	isLast: boolean;
}

export enum Operation {
	Equal = 0,
	Insert = 1,
	Delete = -1,
	Edit = 2
}

export type DiffRows = { prevRows: Row[]; nextRows: Row[] };
