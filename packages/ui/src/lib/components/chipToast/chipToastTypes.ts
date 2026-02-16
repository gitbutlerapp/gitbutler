export type ChipToastType = 'info' | 'success' | 'warning' | 'danger';

export interface ChipToastButtonConfig {
	label: string;
	action: () => void;
}

export interface ChipToastData {
	id: string;
	message: string;
	type: ChipToastType;
	customButton?: ChipToastButtonConfig;
	showDismiss?: boolean;
}

export interface ChipToastOptions {
	type?: ChipToastType;
	customButton?: ChipToastButtonConfig;
	showDismiss?: boolean;
}
