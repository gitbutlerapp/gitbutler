export type ChipToastType = 'neutral' | 'success' | 'warning' | 'error';

export interface ChipToastData {
	id: string;
	message: string;
	type: ChipToastType;
}

export interface ChipToastOptions {
	type?: ChipToastType;
}
